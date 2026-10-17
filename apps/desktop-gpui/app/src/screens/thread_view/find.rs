// SPDX-License-Identifier: AGPL-3.0-only
//! Find in thread (`chat/find/`): counts over every row with every fold open, paints only the rows on screen.

use std::collections::HashMap;
use std::ops::Range;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::text::{RangeHighlight, RenderedText, TextViewState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, BoxShadow, Context, Entity, FontWeight, KeyDownEvent, Pixels, SharedString, Subscription, Window,
};

use super::markdown;
use crate::thread_view::ThreadView;
use crate::transcript::{Row, RowKind};
use crate::ui::{shortcut_tooltip, ActivePalette, Button, Glyph, Palette};

/// `FIND_WORDS`.
pub const FIELD: &str = "Find in thread";
pub const NO_RESULTS: &str = "No results";
pub const INVALID: &str = "This is not a valid regular expression.";
/// `MATCH_CAP`: counting stops here and the count reads "9,999+".
pub const MATCH_CAP: usize = 9_999;
/// `FIELD_MAX`: the query, typed or seeded from a selection, is cut to this many characters.
const FIELD_MAX: usize = 200;
/// `max-w-105`, `inset-x-3 top-3`, `rounded-(--radius-popover)`.
const BAR_WIDTH: Pixels = px(420.);
const BAR_INSET: Pixels = px(12.);
const BAR_RADIUS: Pixels = px(12.);
/// Where a step stands the row holding its match: 40% down the viewport.
pub const SHOW_AT: f32 = 0.4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// What a query matches: a literal lowercased once when case is ignored, or a compiled pattern.
pub enum Needle {
    Literal { text: String, match_case: bool, whole_word: bool },
    Pattern { pattern: regex::Regex, whole_word: bool },
}

/// `needleOf`: None for an empty query, Err for a pattern that does not parse.
pub fn needle(query: &str, o: Options) -> Result<Option<Needle>, regex::Error> {
    if query.is_empty() {
        return Ok(None);
    }
    if !o.regex {
        let text = if o.match_case { query.to_owned() } else { query.to_lowercase() };
        return Ok(Some(Needle::Literal { text, match_case: o.match_case, whole_word: o.whole_word }));
    }
    regex::RegexBuilder::new(query)
        .case_insensitive(!o.match_case)
        .build()
        .map(|pattern| Some(Needle::Pattern { pattern, whole_word: o.whole_word }))
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn bounded(text: &str, r: &Range<usize>) -> bool {
    r.end > r.start && !is_word(text[..r.start].chars().next_back()) && !is_word(text[r.end..].chars().next())
}

/// `spansIn`: every match in one line, at most `room` of them, as byte ranges of `text`.
pub fn spans(text: &str, needle: &Needle, room: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if room == 0 {
        return out;
    }
    match needle {
        Needle::Literal { text: want, match_case, whole_word } => {
            // The haystack lowercased, with the byte of `text` each of its bytes came from.
            let (hay, from): (String, Vec<usize>) = if *match_case {
                (text.to_owned(), (0..=text.len()).collect())
            } else {
                let mut hay = String::with_capacity(text.len());
                let mut from = Vec::with_capacity(text.len() + 1);
                for (at, c) in text.char_indices() {
                    for low in c.to_lowercase() {
                        hay.push(low);
                        from.resize(hay.len(), at);
                    }
                }
                from.push(text.len());
                (hay, from)
            };
            let mut at = 0;
            while let Some(found) = hay[at..].find(want.as_str()) {
                let start = at + found;
                let end = start + want.len();
                let range = from[start]..from[end];
                at = start + hay[start..].chars().next().map_or(1, char::len_utf8).max(want.len());
                if *whole_word && !bounded(text, &range) {
                    continue;
                }
                out.push(range);
                if out.len() >= room || at >= hay.len() {
                    break;
                }
            }
        }
        Needle::Pattern { pattern, whole_word } => {
            for found in pattern.find_iter(text) {
                let range = found.range();
                if range.is_empty() || (*whole_word && !bounded(text, &range)) {
                    continue;
                }
                out.push(range);
                if out.len() >= room {
                    break;
                }
            }
        }
    }
    out
}

/// `FIND_WORDS.count`: "3 of 12", with a plus where counting stopped.
pub fn count(at: usize, total: usize, more: bool) -> String {
    fn grouped(n: usize) -> String {
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
    format!("{} of {}{}", grouped(at), grouped(total), if more { "+" } else { "" })
}

/// The lines a message's markdown renders to, in order; marks, link targets, images, math and mermaid count nothing.
pub fn segments(source: &str, person: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, out: &mut Vec<String>| {
        if !paragraph.is_empty() {
            out.push(paragraph.join(" "));
            paragraph.clear();
        }
    };
    let mut fence: Option<(char, usize, bool)> = None;
    let mut math = false;
    for raw in source.lines() {
        let line = raw.trim();
        if let Some((c, n, shown)) = fence {
            let run = line.chars().take_while(|&d| d == c).count();
            if run >= n && line[run * c.len_utf8()..].trim().is_empty() {
                fence = None;
            } else if shown && !raw.trim().is_empty() {
                out.push(raw.to_owned());
            }
            continue;
        }
        if math {
            math = line != "$$";
            continue;
        }
        let marker = line.chars().next().filter(|c| *c == '`' || *c == '~');
        if let Some(c) = marker {
            let run = line.chars().take_while(|&d| d == c).count();
            if run >= 3 {
                flush(&mut paragraph, &mut out);
                let lang = line[run..].trim();
                fence = Some((c, run, lang != "mermaid"));
                continue;
            }
        }
        if line == "$$" || (line.starts_with("$$") && !line.ends_with("$$")) {
            flush(&mut paragraph, &mut out);
            math = true;
            continue;
        }
        if line.is_empty() {
            flush(&mut paragraph, &mut out);
            continue;
        }
        if is_rule(line) || is_table_rule(line) || (line.starts_with("$$") && line.ends_with("$$") && line.len() > 3) {
            flush(&mut paragraph, &mut out);
            continue;
        }
        let (block, rest) = block_start(line);
        if block {
            flush(&mut paragraph, &mut out);
        }
        if rest.starts_with('|') {
            for cell in rest.trim_matches('|').split('|') {
                let cell = inline(cell.trim(), person);
                if !cell.trim().is_empty() {
                    out.push(cell);
                }
            }
            continue;
        }
        let text = inline(rest, person);
        if text.trim().is_empty() {
            continue;
        }
        if person || heading(line) {
            out.push(text);
        } else {
            paragraph.push(text);
        }
    }
    flush(&mut paragraph, &mut out);
    out.retain(|s| !s.trim().is_empty());
    out
}

fn heading(line: &str) -> bool {
    let line = line.trim_start_matches(['>', ' ']);
    let hashes = line.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

fn is_rule(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && ['-', '*', '_'].iter().any(|m| chars.iter().all(|c| c == m))
}

fn is_table_rule(line: &str) -> bool {
    line.contains('-') && line.contains('|') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Whether a line opens a block of its own, and the line past its markers: a heading, a quote, a list item.
fn block_start(line: &str) -> (bool, &str) {
    let mut rest = line;
    let mut block = false;
    while let Some(r) = rest.strip_prefix('>') {
        rest = r.trim_start();
    }
    let hashes = rest.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && rest[hashes..].starts_with(' ') {
        return (true, rest[hashes..].trim().trim_end_matches('#').trim_end());
    }
    for bullet in ["- ", "* ", "+ "] {
        if let Some(r) = rest.strip_prefix(bullet) {
            rest = r.trim_start();
            block = true;
            break;
        }
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") ")) {
        rest = rest[digits + 2..].trim_start();
        block = true;
    }
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(r) = rest.strip_prefix(task) {
            rest = r;
        }
    }
    (block, rest)
}

/// A line's inline markdown read to the text it renders; a reply's HTML tags drop out, a person's stay as typed.
fn inline(text: &str, person: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() => {
                out.push(chars[i + 1]);
                i += 2;
            }
            '`' => {
                let run = chars[i..].iter().take_while(|&&d| d == '`').count();
                let close = (i + run..chars.len())
                    .find(|&j| chars[j..].iter().take_while(|&&d| d == '`').count() == run && (j == 0 || chars[j - 1] != '`'));
                match close {
                    Some(j) => {
                        let code: String = chars[i + run..j].iter().collect();
                        out.push_str(
                            code.strip_prefix(' ').and_then(|c| c.strip_suffix(' ')).filter(|c| !c.trim().is_empty()).unwrap_or(&code),
                        );
                        i = j + run;
                    }
                    None => {
                        out.extend(&chars[i..i + run]);
                        i += run;
                    }
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => match link(&chars, i + 1) {
                Some((_, end)) => i = end,
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '[' => match link(&chars, i) {
                Some((label, end)) => {
                    out.push_str(&inline(&label, person));
                    i = end;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '<' if !person => match chars[i..].iter().position(|&d| d == '>') {
                Some(n) => {
                    let inner: String = chars[i + 1..i + n].iter().collect();
                    if inner.contains("://") || inner.starts_with("mailto:") {
                        out.push_str(&inner);
                    }
                    i += n + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '*' | '~' => i += 1,
            '_' => {
                let before = i.checked_sub(1).map(|j| chars[j]);
                let after = chars.get(i + 1).copied();
                let inside = before.is_some_and(char::is_alphanumeric) && after.is_some_and(char::is_alphanumeric);
                if inside {
                    out.push(c);
                }
                i += 1;
            }
            '$' if !person => match chars[i + 1..].iter().position(|&d| d == '$') {
                Some(n) if n > 0 && !chars[i + 1].is_whitespace() && !chars[i + n].is_whitespace() => i += n + 2,
                _ => {
                    out.push(c);
                    i += 1;
                }
            },
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// A `[label](target)` or `[label][ref]` starting at `open`: its label and the index past it.
fn link(chars: &[char], open: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    let mut close = None;
    for (j, &c) in chars.iter().enumerate().skip(open) {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let label: String = chars[open + 1..close].iter().collect();
    let (opener, closer) = match chars.get(close + 1) {
        Some('(') => ('(', ')'),
        Some('[') => ('[', ']'),
        _ => return None,
    };
    let mut depth = 0;
    for (j, &c) in chars.iter().enumerate().skip(close + 1) {
        if c == opener {
            depth += 1;
        } else if c == closer {
            depth -= 1;
            if depth == 0 {
                return Some((label, j + 1));
            }
        }
    }
    None
}

/// One row's searched text with every fold open, and what opens it.
struct Doc {
    key: String,
    /// The turn whose fold hides the row, and the group whose list holds it.
    turn: Option<String>,
    group: Option<String>,
    /// A call to open so its body shows, for a match in a tool call's body.
    call: Option<String>,
    /// A tool call's text, searched with Include tool calls on and never painted.
    tool: bool,
    segments: Vec<String>,
}

/// One match: the row it is in and its place among that row's matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub key: String,
    pub ordinal: usize,
}

/// A body's highlights as last set: the rendered text they index and what they paint.
struct Painted {
    text: RenderedText,
    stamp: u64,
    ranges: Vec<Range<usize>>,
    current: Option<usize>,
}

/// Find's own state, kept by the thread view while it shows a thread.
#[derive(Default)]
pub struct Find {
    open: bool,
    input: Option<Entity<InputState>>,
    query: String,
    options: Options,
    tools: bool,
    docs: Vec<Doc>,
    /// The transcript's event count the docs were read at, so a live event reads them again.
    read_at: usize,
    hits: Vec<(usize, usize)>,
    capped: bool,
    invalid: bool,
    current: Option<usize>,
    /// The current match was picked fresh and not yet stepped to, so the first step shows it where it is.
    fresh: bool,
    /// The current match's row was just stood in view; its line is revealed once the row stands still.
    reveal: bool,
    stamp: u64,
    painted: HashMap<String, Painted>,
    _events: Option<Subscription>,
}

impl Find {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// "3 of 12", "No results", or nothing for an empty query.
    pub fn status(&self) -> Option<String> {
        if self.query.is_empty() || self.invalid {
            return None;
        }
        if self.hits.is_empty() {
            return Some(NO_RESULTS.to_owned());
        }
        Some(count(self.current.map_or(0, |c| c + 1), self.hits.len(), self.capped))
    }

    pub fn current(&self) -> Option<Hit> {
        self.current.and_then(|c| self.hit(c))
    }

    /// The ranges last painted on the row keyed `key`, and which of them is the current match.
    pub fn painted(&self, key: &str) -> Option<(Vec<Range<usize>>, Option<usize>)> {
        self.painted.get(key).map(|p| (p.ranges.clone(), p.current))
    }

    fn hit(&self, ix: usize) -> Option<Hit> {
        self.hits.get(ix).map(|&(doc, ordinal)| Hit { key: self.docs[doc].key.clone(), ordinal })
    }

    fn needle(&self) -> Option<Needle> {
        needle(&self.query, self.options).ok().flatten()
    }
}

/// The docs for `rows`, every fold and group open.
fn docs_of(rows: &[Row]) -> Vec<Doc> {
    let mut docs = Vec::new();
    let mut group: Option<String> = None;
    for row in rows {
        let turn = row.turn.clone();
        match &row.kind {
            RowKind::Person { .. } => docs.push(Doc {
                key: row.key.clone(),
                turn,
                group: None,
                call: None,
                tool: false,
                segments: segments(&markdown::person_text(&row.text), true),
            }),
            RowKind::Agent { .. } => {
                docs.push(Doc { key: row.key.clone(), turn, group: None, call: None, tool: false, segments: segments(&row.text, false) })
            }
            RowKind::Group { group: g, .. } | RowKind::Live { group: g, .. } => group = Some(g.clone()),
            RowKind::Calls { group: g, calls } => {
                for call in calls {
                    let label = match &call.verb {
                        Some(verb) => format!("{verb} {}", call.label),
                        None => call.label.clone(),
                    };
                    let mut segments = vec![label.replace('\n', " ")];
                    segments.extend(call.command.iter().map(|c| c.replace('\n', " ")));
                    segments.extend(call.body.iter().flat_map(|b| b.lines().map(str::to_owned)));
                    segments.retain(|s| !s.trim().is_empty());
                    let call_key = (call.body.is_some() || call.patch.is_some()).then(|| call.key.clone());
                    docs.push(Doc {
                        key: row.key.clone(),
                        turn: turn.clone(),
                        group: Some(g.clone()),
                        call: call_key,
                        tool: true,
                        segments,
                    });
                }
            }
            RowKind::Work { .. } => docs.push(Doc {
                key: row.key.clone(),
                turn,
                group: group.clone(),
                call: None,
                tool: true,
                segments: vec![row.text.replace('\n', " ")],
            }),
            _ => {}
        }
    }
    docs.retain(|d| !d.segments.is_empty());
    docs
}

/// Opens the bar, or takes focus back to it with its text selected; a one-line selection in a reply seeds the query.
pub fn open(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) {
    let seed = view.bodies().values().map(|b| b.read(cx).selected_text()).find(|t| !t.trim().is_empty()).map(|t| t.trim().to_owned());
    let seed = seed.filter(|s| !s.contains('\n')).map(|s| s.chars().take(FIELD_MAX).collect::<String>());
    let input = match view.find().input.clone() {
        Some(input) => input,
        None => {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(FIELD));
            let events = cx.subscribe_in(&input, window, |view: &mut ThreadView, input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let mut query = input.read(cx).value().to_string();
                    if query.chars().count() > FIELD_MAX {
                        query = query.chars().take(FIELD_MAX).collect();
                    }
                    if view.find().query != query {
                        view.find().query = query;
                        search(view, true, cx);
                    }
                }
                InputEvent::PressEnter { shift, .. } => step(view, if *shift { 1 } else { -1 }, window, cx),
                _ => {}
            });
            let find = view.find();
            find.input = Some(input.clone());
            find._events = Some(events);
            input
        }
    };
    if let Some(seed) = seed {
        input.update(cx, |i, cx| i.set_value(seed.clone(), window, cx));
        view.find().query = seed;
    }
    let reopened = !view.find().open;
    view.find().open = true;
    input.update(cx, |i, cx| {
        i.focus(window, cx);
        i.select_all(window, cx);
    });
    if reopened {
        view.find().read_at = usize::MAX;
        search(view, true, cx);
    }
    cx.notify();
}

/// Closes the bar, takes its highlights off and hands focus back to the transcript.
pub fn close(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) {
    let find = view.find();
    find.open = false;
    find.docs.clear();
    find.hits.clear();
    find.current = None;
    let painted: Vec<String> = find.painted.drain().map(|(k, _)| k).collect();
    for key in painted {
        if let Some(body) = view.bodies().get(&key).cloned() {
            body.update(cx, |b, cx| b.clear_range_highlights(cx));
        }
    }
    view.focus_transcript(window, cx);
    cx.notify();
}

pub fn set_option(view: &mut ThreadView, pick: impl FnOnce(&mut Find), cx: &mut Context<ThreadView>) {
    pick(view.find());
    search(view, true, cx);
}

/// Searches again; a fresh pick is the newest match at or above the last row the reader sees.
fn search(view: &mut ThreadView, fresh: bool, cx: &mut Context<ThreadView>) {
    if !view.find().open {
        return;
    }
    let was = view.find_ref().current().filter(|_| !fresh);
    let events = view.events_seen();
    if view.find().read_at != events {
        let rows = view.unfolded();
        let find = view.find();
        find.docs = docs_of(&rows);
        find.read_at = events;
    }
    let anchor = view.last_seen_row().map(|key| key.to_owned());
    let find = view.find();
    find.stamp += 1;
    find.hits.clear();
    find.capped = false;
    find.invalid = false;
    match needle(&find.query, find.options) {
        Err(_) => find.invalid = true,
        Ok(None) => {}
        Ok(Some(needle)) => {
            'docs: for (d, doc) in find.docs.iter().enumerate() {
                if doc.tool && !find.tools {
                    continue;
                }
                let mut ordinal = 0;
                for segment in &doc.segments {
                    for _ in spans(segment, &needle, MATCH_CAP + 1 - find.hits.len()) {
                        find.hits.push((d, ordinal));
                        ordinal += 1;
                        if find.hits.len() > MATCH_CAP {
                            find.hits.truncate(MATCH_CAP);
                            find.capped = true;
                            break 'docs;
                        }
                    }
                }
            }
        }
    }
    let keyed = |find: &Find, key: &str| find.docs.iter().position(|d| d.key == key);
    let kept = was.and_then(|was| find.hits.iter().position(|&(d, o)| find.docs[d].key == was.key && o == was.ordinal));
    find.current = if find.hits.is_empty() {
        None
    } else if kept.is_some() {
        kept
    } else {
        let at = anchor.as_deref().and_then(|k| keyed(find, k));
        Some(match at {
            Some(at) => find.hits.iter().rposition(|&(d, _)| d <= at).unwrap_or(0),
            None => find.hits.len() - 1,
        })
    };
    find.fresh = fresh;
    cx.notify();
}

/// `step`: -1 for the older match, 1 for the newer; a freshly picked match not yet shown is shown first.
pub fn step(view: &mut ThreadView, direction: isize, _: &mut Window, cx: &mut Context<ThreadView>) {
    let find = view.find();
    let total = find.hits.len();
    if total == 0 {
        return;
    }
    let unseen = find.fresh && find.current.and_then(|c| find.hit(c)).is_some_and(|hit| !in_sight(view, &hit.key));
    let find = view.find();
    let next = match find.current {
        Some(current) if unseen => current,
        Some(current) => (current as isize + direction).rem_euclid(total as isize) as usize,
        None if direction < 0 => total - 1,
        None => 0,
    };
    find.current = Some(next);
    find.fresh = false;
    find.stamp += 1;
    show(view, cx);
}

/// Whether the row keyed `key` is drawn inside the viewport; a folded row is not.
fn in_sight(view: &ThreadView, key: &str) -> bool {
    let list = view.list();
    view.rows()
        .iter()
        .position(|r| r.key == key)
        .is_some_and(|ix| list.item_is_above_viewport(ix) == Some(false) && list.item_is_below_viewport(ix) == Some(false))
}

/// Opens what hides the current match and stands its row at 40% of the viewport.
fn show(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let find = view.find();
    let Some((doc, _)) = find.current.and_then(|c| find.hits.get(c).copied()) else { return };
    let (key, turn, group, call) = {
        let d = &find.docs[doc];
        (d.key.clone(), d.turn.clone(), d.group.clone(), d.call.clone())
    };
    let shown = |view: &ThreadView| view.rows().iter().position(|r| r.key == key);
    if shown(view).is_none() {
        if let Some(turn) =
            turn.filter(|t| view.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: false }) && r.turn.as_deref() == Some(t)))
        {
            view.toggle(&turn, cx);
        }
    }
    if shown(view).is_none() {
        if let Some(group) = group {
            view.toggle(&group, cx);
        }
    }
    if let Some(call) = call {
        let closed = view.rows().iter().any(|r| match &r.kind {
            RowKind::Calls { calls, .. } => calls.iter().any(|c| c.key == call && !c.open),
            _ => false,
        });
        if closed {
            view.toggle(&call, cx);
        }
    }
    if let Some(ix) = shown(view) {
        let height = view.list().viewport_bounds().size.height;
        view.reveal(ix, height * SHOW_AT, cx);
        view.find().reveal = true;
    }
    cx.notify();
}

/// Paints the matches of the rows on screen as the view draws; a body is painted again only when its text or matches moved.
pub fn paint(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    if !view.find().open {
        return;
    }
    if view.find().read_at != view.events_seen() && view.find().read_at != usize::MAX {
        search(view, false, cx);
    }
    let Some(needle) = view.find().needle() else {
        if !view.find().painted.is_empty() {
            let keys: Vec<String> = view.find().painted.drain().map(|(k, _)| k).collect();
            let bodies: Vec<_> = keys.iter().filter_map(|k| view.bodies().get(k).cloned()).collect();
            cx.defer(move |cx| bodies.into_iter().for_each(|b| b.update(cx, |b, cx| b.clear_range_highlights(cx))));
        }
        return;
    };
    let p = cx.palette().clone();
    let (all, now) = (p.primary.opacity(0.2), p.primary.opacity(0.45));
    let revealing = view.is_revealing();
    let find = view.find_ref();
    let current = find.current.and_then(|c| find.hit(c));
    let mut wanted: std::collections::HashSet<String> = find.hits.iter().map(|&(d, _)| find.docs[d].key.clone()).collect();
    wanted.extend(find.painted.keys().cloned());
    let bodies: Vec<(String, Entity<TextViewState>)> =
        view.bodies().iter().filter(|(k, _)| wanted.contains(k.as_str())).map(|(k, b)| (k.clone(), b.clone())).collect();
    let mut work: Vec<Paint> = Vec::new();
    let stamp = view.find_ref().stamp;
    let want_reveal = view.find_ref().reveal && !revealing;
    for (key, body) in bodies {
        let text = body.read(cx).rendered_text();
        let unchanged = view.find_ref().painted.get(&key).is_some_and(|p| p.text == text && p.stamp == stamp);
        let is_current = current.as_ref().is_some_and(|c| c.key == key);
        if unchanged && !(is_current && want_reveal) {
            continue;
        }
        let ranges = rendered_spans(text.as_str(), &needle);
        let pick = current.as_ref().filter(|c| c.key == key).map(|c| c.ordinal.min(ranges.len().saturating_sub(1)));
        let reveal = if is_current && want_reveal { pick.and_then(|i| ranges.get(i).cloned()) } else { None };
        let highlights =
            ranges.iter().enumerate().map(|(i, r)| RangeHighlight::new(r.clone(), if Some(i) == pick { now } else { all })).collect();
        if reveal.is_some() {
            view.find().reveal = false;
        }
        view.find().painted.insert(key, Painted { text, stamp, ranges, current: pick });
        work.push((body, highlights, reveal));
    }
    if !work.is_empty() {
        cx.defer(move |cx| {
            for (body, highlights, reveal) in work {
                body.update(cx, |b, cx| {
                    _ = b.set_range_highlights(highlights, cx);
                    if let Some(range) = reveal {
                        _ = b.reveal_range(range, cx);
                    }
                });
            }
        });
    }
}

/// A body to paint: its highlights and the range to reveal.
type Paint = (Entity<TextViewState>, Vec<RangeHighlight>, Option<Range<usize>>);

/// Every match in a body's rendered text, line by line as the model cuts it.
pub fn rendered_spans(text: &str, needle: &Needle) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split('\n') {
        if !line.trim().is_empty() {
            out.extend(spans(line, needle, usize::MAX).into_iter().map(|r| r.start + at..r.end + at));
        }
        at += line.len() + 1;
    }
    out
}

/// Escape closes the bar from anywhere inside it.
pub fn key_down(view: &mut ThreadView, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<ThreadView>) {
    if event.keystroke.key == "escape" && view.find().open {
        cx.stop_propagation();
        close(view, window, cx);
    }
}

/// `dropdown-glass popover-shadow`: the popover ground, at 96% under an 8 px blur off the Mac.
fn glass(el: gpui_kit::Div, p: &Palette, dark: bool) -> gpui_kit::Div {
    let shadow = if dark {
        BoxShadow {
            color: gpui_kit::black().opacity(0.8),
            offset: gpui_kit::point(px(0.), px(18.)),
            blur_radius: px(44.),
            spread_radius: px(-18.),
            inset: false,
        }
    } else {
        BoxShadow {
            color: gpui_kit::black().opacity(0.55),
            offset: gpui_kit::point(px(0.), px(16.)),
            blur_radius: px(40.),
            spread_radius: px(-18.),
            inset: false,
        }
    };
    let el = el.border_1().border_color(p.foreground.opacity(0.1)).shadow(vec![shadow]);
    if cfg!(target_os = "macos") {
        el.bg(p.popover)
    } else {
        el.bg(p.popover.opacity(0.96)).backdrop_blur(px(8.))
    }
}

/// The bar: the field with its four toggles at its end, the count, older, newer and close.
pub fn bar(view: &mut ThreadView, window: &Window, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    let find = view.find_ref();
    if !find.open {
        return None;
    }
    let input = find.input.clone()?;
    let focused = gpui_kit::Focusable::focus_handle(input.read(cx), cx).is_focused(window);
    let p = cx.palette().clone();
    let dark = cx.theme().is_dark();
    let status = find.status();
    let can_step = !find.hits.is_empty();
    let pressed = [find.options.match_case, find.options.whole_word, find.options.regex, find.tools];
    let toggles = [
        (Glyph::CaseSensitive, "Match case"),
        (Glyph::WholeWord, "Match whole word"),
        (Glyph::Regex, "Use regular expression"),
        (Glyph::Wrench, "Include tool calls"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (glyph, label))| {
        let on = pressed[i];
        div()
            .id(SharedString::from(format!("find-toggle-{i}")))
            .debug_selector(move || format!("find-toggle-{i}"))
            .tooltip(shortcut_tooltip(label, &[]))
            .flex_none()
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .cursor_pointer()
            .when(on, |el| el.bg(p.accent))
            .hover(|s| s.bg(p.accent))
            .child(Icon::new(glyph).with_size(px(14.)).text_color(p.foreground.opacity(0.8)))
            .on_click(cx.listener(move |view, _, _, cx| {
                set_option(
                    view,
                    |f| match i {
                        0 => f.options.match_case = !f.options.match_case,
                        1 => f.options.whole_word = !f.options.whole_word,
                        2 => f.options.regex = !f.options.regex,
                        _ => f.tools = !f.tools,
                    },
                    cx,
                )
            }))
    })
    .collect::<Vec<_>>();
    let field = Input::new(&input)
        .appearance(false)
        .with_size(gpui_kit::component::Size::Small)
        .suffix(h_flex().gap_0().mr(px(-16.)).children(toggles));
    let row = h_flex()
        .h(px(36.))
        .p(px(4.))
        .gap(px(4.))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .h(px(28.))
                .rounded(px(8.))
                .border_1()
                .map(|el| if focused { el.border_color(p.ring).bg(p.background) } else { el.border_color(gpui_kit::transparent_black()) })
                .pl(px(1.))
                .pr(px(9.))
                .text_size(px(14.))
                .child(field),
        )
        .child(
            h_flex()
                .debug_selector(|| "find-count".into())
                .w(px(96.))
                .flex_none()
                .justify_end()
                .gap(px(6.))
                .whitespace_nowrap()
                .text_size(px(13.))
                .text_color(p.muted_foreground)
                .children(status),
        )
        .child(
            div().debug_selector(|| "find-older".into()).child(
                Button::icon_xs("find-older", Glyph::ChevronUp)
                    .held(!can_step)
                    .on_click(cx.listener(|view, _, window, cx| step(view, -1, window, cx))),
            ),
        )
        .child(
            div().debug_selector(|| "find-newer".into()).child(
                Button::icon_xs("find-newer", Glyph::ChevronDown)
                    .held(!can_step)
                    .on_click(cx.listener(|view, _, window, cx| step(view, 1, window, cx))),
            ),
        )
        .child(
            div()
                .debug_selector(|| "find-close".into())
                .child(Button::icon_xs("find-close", Glyph::Close).on_click(cx.listener(|view, _, window, cx| close(view, window, cx)))),
        );
    let note = find
        .invalid
        .then(|| div().px(px(12.)).pb(px(8.)).text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(INVALID));
    Some(
        h_flex()
            .absolute()
            .top(BAR_INSET)
            .left(BAR_INSET)
            .right(BAR_INSET)
            .justify_end()
            .child(
                glass(v_flex(), &p, dark)
                    .id("find-bar")
                    .debug_selector(|| "find-bar".into())
                    .w_full()
                    .max_w(BAR_WIDTH)
                    .rounded(BAR_RADIUS)
                    .font_weight(FontWeight::NORMAL)
                    .occlude()
                    .capture_key_down(cx.listener(key_down))
                    .child(row)
                    .children(note),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(q: &str) -> Needle {
        needle(q, Options::default()).unwrap().unwrap()
    }

    #[test]
    fn a_literal_matches_without_case_and_a_whole_word_only_between_word_edges() {
        assert_eq!(spans("Foo food FOO", &lit("foo"), 10), vec![0..3, 4..7, 9..12]);
        let word = needle("foo", Options { whole_word: true, ..Options::default() }).unwrap().unwrap();
        assert_eq!(spans("Foo food FOO", &word, 10), vec![0..3, 9..12]);
        let case = needle("Foo", Options { match_case: true, ..Options::default() }).unwrap().unwrap();
        assert_eq!(spans("Foo food FOO", &case, 10), vec![0..3]);
        assert_eq!(spans("ÄBC äbc", &lit("äbc"), 10), vec![0..4, 5..9]);
    }

    #[test]
    fn a_pattern_matches_and_a_broken_one_is_refused() {
        let pattern = needle("fo+d?", Options { regex: true, ..Options::default() }).unwrap().unwrap();
        assert_eq!(spans("fo food", &pattern, 10), vec![0..2, 3..7]);
        assert!(needle("(", Options { regex: true, ..Options::default() }).is_err());
    }

    #[test]
    fn the_count_reads_as_the_web_writes_it() {
        assert_eq!(count(3, 12, false), "3 of 12");
        assert_eq!(count(1, 9_999, true), "1 of 9,999+");
    }

    #[test]
    fn markdown_reads_to_the_lines_it_renders() {
        let md = "# Title\n\nSome **bold** and `code` with [a link](http://x).\nSecond line.\n\n- item one\n- item _two_\n\n```rust\nlet a = 1;\n```\n\n| a | b |\n|---|---|\n| c | d |";
        assert_eq!(
            segments(md, false),
            vec!["Title", "Some bold and code with a link. Second line.", "item one", "item two", "let a = 1;", "a", "b", "c", "d"]
        );
        assert_eq!(segments("one\ntwo", true), vec!["one", "two"]);
    }
}
