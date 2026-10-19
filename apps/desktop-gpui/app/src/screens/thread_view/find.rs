// SPDX-License-Identifier: AGPL-3.0-only
//! Find in thread (`chat/find/`): counts over every row with every fold open, paints only the rows on screen. A row's
//! text is read again only when the row changed, so a live event while the bar is open reads only its own turn.

use std::collections::HashMap;
use std::ops::Range;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::text::{RangeHighlight, RenderedText, TextViewState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, AnyView, App, Context, Entity, FontWeight, HighlightStyle, KeyDownEvent, Pixels, SharedString,
    Subscription, WeakFocusHandle, Window,
};

use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::{Row, RowKind};
use crate::ui::{ActivePalette, Button, Glyph, Palette};

/// `FIND_WORDS`.
pub const FIELD: &str = "Find in thread";
pub const NO_RESULTS: &str = "No results";
pub const INVALID: &str = "This is not a valid regular expression.";
pub const TRIMMED: &str = "Older messages are past what wsp keeps.";
/// `MATCH_CAP`: counting stops here and the count reads "9,999+".
pub const MATCH_CAP: usize = 9_999;
/// `FIELD_MAX`: the query, typed or seeded from a selection, is cut to this many characters.
const FIELD_MAX: usize = 200;
/// `max-w-105`, `inset-x-3 top-3`, `rounded-(--radius-popover)`.
const BAR_WIDTH: Pixels = px(420.);
const BAR_INSET: Pixels = px(12.);
const BAR_RADIUS: Pixels = px(12.);
/// Tailwind's `sm`, under which the bar puts its toggles on their own row and narrows its count to `w-18`.
const NARROW: Pixels = px(640.);
/// Where a step stands the row holding its match: 40% down the viewport.
pub const SHOW_AT: f32 = 0.4;
/// `BAR_CLEARANCE`, and the room a match keeps over the docked composer past its height: where a step's line stands.
const BAR_CLEARANCE: Pixels = px(60.);
const COMPOSER_CLEARANCE: Pixels = px(16.);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// What a query matches: a literal lowercased once when case is ignored, or a compiled pattern.
#[derive(Clone)]
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

/// The lines a message's markdown renders to, in order; marks, link targets, images and math count nothing.
pub fn segments(source: &str, person: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, out: &mut Vec<String>| {
        if !paragraph.is_empty() {
            out.push(paragraph.join(" "));
            paragraph.clear();
        }
    };
    let mut fence: Option<(char, usize)> = None;
    let mut math = false;
    for raw in source.lines() {
        let line = raw.trim();
        if let Some((c, n)) = fence {
            let run = line.chars().take_while(|&d| d == c).count();
            if run >= n && line[run * c.len_utf8()..].trim().is_empty() {
                fence = None;
            } else if !raw.trim().is_empty() {
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
                fence = Some((c, run));
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

/// One part of a row's searched text with every fold open, and what opens it.
#[derive(Clone)]
struct Doc {
    key: String,
    /// Which part of the row: empty for its markdown body, else the name `marks` is asked by for the plain text drawn.
    part: String,
    /// The turn whose fold hides the row, and the group whose list holds it.
    turn: Option<String>,
    group: Option<String>,
    /// A call to open so its body shows, for a match in a tool call's body.
    call: Option<String>,
    /// What `toggle` opens when the row stands shut over the part: a plan's fold, a subagent's, a work row's body.
    opens: Option<String>,
    /// Searched only with Include tool calls on: a tool row, a subagent's own tool lines.
    tool: bool,
    segments: Vec<String>,
}

/// One match: the row it is in, the part of it, and its place among that part's matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub key: String,
    pub part: String,
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
    /// The view's row revision the docs were read at, so any change to the rows reads them again.
    read_at: usize,
    /// Each row's docs as last read, with the row and the group above it they were read from.
    read: HashMap<String, (Row, Option<String>, Vec<Doc>)>,
    /// How many rows have been read to docs since the view was made.
    reads: usize,
    hits: Vec<(usize, usize)>,
    /// The doc of each row and part that holds a match, which `marks` paints.
    hit_docs: HashMap<(String, String), usize>,
    needle: Option<Needle>,
    capped: bool,
    invalid: bool,
    /// Older pages are still being read in, so the count says "+" and no "No results" yet.
    partial: bool,
    /// The host's caps dropped older events of the thread.
    trimmed: bool,
    current: Option<usize>,
    /// The current match was picked fresh and not yet stepped to, so the first step shows it where it is.
    fresh: bool,
    /// The current match's row was just stood in view; its line is revealed once the row stands still.
    reveal: bool,
    stamp: u64,
    painted: HashMap<String, Painted>,
    /// Another thread showed under the open bar: its first search once loaded picks its newest match.
    newest: bool,
    /// What held focus as the bar opened, which takes it back as the bar closes.
    back: Option<WeakFocusHandle>,
    _events: Option<Subscription>,
}

impl Find {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the find field holds the window's focus.
    pub(crate) fn typing(&self, window: &Window, cx: &App) -> bool {
        self.input.as_ref().is_some_and(|input| gpui_kit::Focusable::focus_handle(input.read(cx), cx).is_focused(window))
    }

    /// "3 of 12", "No results", or nothing for an empty query; "+" and no "No results" while older pages come in.
    pub fn status(&self) -> Option<String> {
        if self.query.is_empty() || self.invalid {
            return None;
        }
        if self.hits.is_empty() {
            return (!self.partial).then(|| NO_RESULTS.to_owned());
        }
        Some(count(self.current.map_or(0, |c| c + 1), self.hits.len(), self.capped || self.partial))
    }

    /// The line under the bar: a pattern that does not parse, else a thread the host's caps cut.
    pub fn note(&self) -> Option<&'static str> {
        if self.invalid {
            Some(INVALID)
        } else if self.trimmed {
            Some(TRIMMED)
        } else {
            None
        }
    }

    /// How many rows have been read to searched text since the view was made.
    pub fn reads(&self) -> usize {
        self.reads
    }

    pub fn current(&self) -> Option<Hit> {
        self.current.and_then(|c| self.hit(c))
    }

    /// The ranges last painted on the row keyed `key`, and which of them is the current match.
    pub fn painted(&self, key: &str) -> Option<(Vec<Range<usize>>, Option<usize>)> {
        self.painted.get(key).map(|p| (p.ranges.clone(), p.current))
    }

    fn hit(&self, ix: usize) -> Option<Hit> {
        self.hits.get(ix).map(|&(doc, ordinal)| Hit { key: self.docs[doc].key.clone(), part: self.docs[doc].part.clone(), ordinal })
    }

    /// Another thread shows: the bar keeps its query and toggles and searches that thread from its newest match.
    pub(crate) fn switch(&mut self) {
        self.docs.clear();
        self.read.clear();
        self.hits.clear();
        self.hit_docs.clear();
        self.painted.clear();
        self.current = None;
        self.capped = false;
        self.reveal = false;
        self.read_at = usize::MAX;
        self.newest = self.open;
    }
}

/// One row's docs with every fold and group open; `group` is the tool group the rows above it opened.
fn row_docs(row: &Row, group: &Option<String>) -> Vec<Doc> {
    let doc = |part: String, segments: Vec<String>| Doc {
        key: row.key.clone(),
        part,
        turn: row.turn.clone(),
        group: None,
        call: None,
        opens: None,
        tool: false,
        segments,
    };
    let line = |text: &str| vec![text.replace('\n', " ")];
    let mut docs = match &row.kind {
        RowKind::Person { .. } => vec![doc(String::new(), segments(&super::rows::body_text(row), true))],
        RowKind::Agent { .. } => vec![doc(String::new(), segments(&super::rows::body_text(row), false))],
        RowKind::Plan { .. } => vec![Doc {
            opens: crate::transcript::plan_folds(&row.text).then(|| format!("plan:{}", row.key)),
            ..doc(String::new(), segments(&super::rows::body_text(row), false))
        }],
        RowKind::Calls { group: g, calls } => calls
            .iter()
            .flat_map(|call| {
                let label = match &call.verb {
                    Some(verb) => format!("{verb} {}", call.label),
                    None => call.label.clone(),
                };
                let opens = (call.body.is_some() || call.patch.is_some()).then(|| call.key.clone());
                let in_group = |part: &str, segments: Vec<String>, opens: Option<String>| Doc {
                    group: Some(g.clone()),
                    call: opens,
                    tool: true,
                    ..doc(format!("{}:{part}", call.key), segments)
                };
                [
                    Some(in_group("label", line(&label), None)),
                    call.command.as_deref().map(|c| in_group("command", line(c), None)),
                    call.body.as_deref().map(|b| in_group("body", b.lines().map(str::to_owned).collect(), opens.clone())),
                ]
            })
            .flatten()
            .collect(),
        RowKind::Work { body, .. } => {
            let mut docs = vec![Doc { group: group.clone(), tool: true, ..doc("label".to_owned(), line(&row.text)) }];
            if let Some(body) = body {
                docs.push(Doc {
                    group: group.clone(),
                    opens: Some(row.key.clone()),
                    tool: true,
                    ..doc("body".to_owned(), body.lines().map(str::to_owned).collect())
                });
            }
            docs
        }
        RowKind::Subagent { id, lines, .. } => lines
            .iter()
            .enumerate()
            .flat_map(|(i, l)| {
                let part =
                    |detail: bool, text: &str| Doc { opens: Some(id.clone()), tool: l.mono, ..doc(subagent_part(i, detail), line(text)) };
                [Some(part(false, &l.text)), l.detail.as_deref().map(|d| part(true, d))]
            })
            .flatten()
            .collect(),
        _ => Vec::new(),
    };
    for d in &mut docs {
        d.segments.retain(|s| !s.trim().is_empty());
    }
    docs.retain(|d| !d.segments.is_empty());
    docs
}

/// The part name of a subagent's line `i`, its label or its detail.
pub fn subagent_part(i: usize, detail: bool) -> String {
    if detail {
        format!("line:{i}:detail")
    } else {
        format!("line:{i}")
    }
}

/// The docs of every row with every fold open, reading again only the rows that changed since `read` held them.
fn docs_of(rows: &[Row], read: &mut HashMap<String, (Row, Option<String>, Vec<Doc>)>, reads: &mut usize) -> Vec<Doc> {
    let mut docs = Vec::new();
    let mut group: Option<String> = None;
    let mut kept = HashMap::with_capacity(rows.len());
    for row in rows {
        if let RowKind::Group { group: g, .. } | RowKind::Live { group: g, .. } = &row.kind {
            group = Some(g.clone());
        }
        let entry = match read.remove(&row.key) {
            Some(held) if held.0 == *row && held.1 == group => held,
            _ => {
                *reads += 1;
                (row.clone(), group.clone(), row_docs(row, &group))
            }
        };
        docs.extend(entry.2.iter().cloned());
        kept.insert(row.key.clone(), entry);
    }
    *read = kept;
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
    if reopened {
        view.find().back = window.focused(cx).map(|focused| focused.downgrade());
    }
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

/// Closes the bar, takes its highlights off and hands focus back to what held it as the bar opened; with nothing there
/// but the transcript's page, the screen gives it to the composer, else the transcript keeps it.
pub fn close(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) {
    let find = view.find();
    find.open = false;
    let back = find.back.take().and_then(|back| back.upgrade());
    find.docs.clear();
    find.hits.clear();
    find.current = None;
    let painted: Vec<String> = find.painted.drain().map(|(k, _)| k).collect();
    for key in painted {
        if let Some(body) = view.bodies().get(&key).cloned() {
            body.update(cx, |b, cx| b.clear_range_highlights(cx));
        }
    }
    // The transcript's own focus is what a click on its page gives, where today's app leaves the body focused.
    match back.filter(|back| *back != gpui_kit::Focusable::focus_handle(view, cx)) {
        Some(back) => back.focus(window, cx),
        // A subagent's page draws no composer: the keys stay with its transcript.
        None => {
            view.focus_transcript(window, cx);
            if !view.paged() {
                cx.emit(ThreadViewEvent::FocusComposer);
            }
        }
    }
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
    let events = view.revision();
    if view.find().read_at != events {
        let rows = view.unfolded();
        let find = view.find();
        find.docs = docs_of(&rows, &mut find.read, &mut find.reads);
        find.read_at = events;
    }
    let loaded = view.loaded();
    let (partial, trimmed) = (!view.complete(), view.trimmed());
    let anchor = if view.find_ref().newest { None } else { anchor_doc(view) };
    let find = view.find();
    find.stamp += 1;
    find.hits.clear();
    find.capped = false;
    find.invalid = false;
    find.partial = partial;
    find.trimmed = trimmed;
    find.needle = None;
    match needle(&find.query, find.options) {
        Err(_) => find.invalid = true,
        Ok(None) => {}
        Ok(Some(needle)) => {
            (find.hits, find.capped) = matches(&find.docs, &needle, find.tools, anchor);
            find.needle = Some(needle);
        }
    }
    find.hit_docs = find.hits.iter().map(|&(d, _)| ((find.docs[d].key.clone(), find.docs[d].part.clone()), d)).collect();
    let kept = was.and_then(|was| {
        find.hits.iter().position(|&(d, o)| find.docs[d].key == was.key && find.docs[d].part == was.part && o == was.ordinal)
    });
    find.current = if find.hits.is_empty() {
        None
    } else if kept.is_some() {
        kept
    } else {
        Some(match anchor {
            Some(at) => find.hits.iter().rposition(|&(d, _)| d <= at).unwrap_or(0),
            None => find.hits.len() - 1,
        })
    };
    find.fresh = fresh || find.newest;
    find.newest &= !loaded;
    cx.notify();
}

/// The doc nearest above the reader: the last doc of the last row they see, or of the nearest row above it with one.
fn anchor_doc(view: &ThreadView) -> Option<usize> {
    let seen = view.last_seen()?;
    let last: HashMap<&str, usize> = view.find_ref().docs.iter().enumerate().map(|(d, doc)| (doc.key.as_str(), d)).collect();
    view.rows()[..=seen].iter().rev().find_map(|r| last.get(r.key.as_str()).copied())
}

/// `searchDocs`: every match in thread order, at most MATCH_CAP of them; past the cap, the ones nearest the reader, read
/// outward from the `anchor` doc (the newest when None), the docs above it before the ones below.
fn matches(docs: &[Doc], needle: &Needle, tools: bool, anchor: Option<usize>) -> (Vec<(usize, usize)>, bool) {
    let mut found = Vec::new();
    let Some(last) = docs.len().checked_sub(1) else { return (found, false) };
    let anchor = anchor.unwrap_or(last).min(last);
    let outward = (0..=last).flat_map(|step| {
        let below = (step > 0).then_some(anchor + step).filter(|&d| d <= last);
        anchor.checked_sub(step).into_iter().chain(below)
    });
    let mut capped = false;
    for d in outward {
        let doc = &docs[d];
        if doc.tool && !tools {
            continue;
        }
        let mut ordinal = 0;
        for segment in &doc.segments {
            for _ in spans(segment, needle, MATCH_CAP + 1 - found.len()) {
                found.push((d, ordinal));
                ordinal += 1;
            }
            if found.len() > MATCH_CAP {
                break;
            }
        }
        if found.len() > MATCH_CAP {
            found.truncate(MATCH_CAP);
            capped = true;
            break;
        }
    }
    found.sort_unstable();
    (found, capped)
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
    let (key, turn, group, call, opens) = {
        let d = &find.docs[doc];
        (d.key.clone(), d.turn.clone(), d.group.clone(), d.call.clone(), d.opens.clone())
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
    if let Some(opens) = opens {
        let shut = view.rows().iter().any(|r| {
            r.key == key
                && matches!(
                    r.kind,
                    RowKind::Plan { open: false, .. } | RowKind::Subagent { open: false, .. } | RowKind::Work { open: false, .. }
                )
        });
        if shut {
            view.toggle(&opens, cx);
        }
    }
    // `useRevealOpen` on a clamped message: a match in it opens it whole.
    let clamped =
        view.rows().iter().any(|r| r.key == key && matches!(r.kind, RowKind::Person { full: false, .. }) && super::rows::clamps(&r.text));
    if clamped {
        view.toggle(&format!("full:{key}"), cx);
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
    if view.find().read_at != view.revision() {
        search(view, false, cx);
    }
    let Some(needle) = view.find().needle.clone() else {
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
    let current = find.current.and_then(|c| find.hit(c)).filter(|c| c.part.is_empty());
    let mut wanted: std::collections::HashSet<String> =
        find.hits.iter().map(|&(d, _)| &find.docs[d]).filter(|d| d.part.is_empty()).map(|d| d.key.clone()).collect();
    wanted.extend(find.painted.keys().cloned());
    let bodies: Vec<(String, Entity<TextViewState>)> =
        view.bodies().iter().filter(|(k, _)| wanted.contains(k.as_str())).map(|(k, b)| (k.clone(), b.clone())).collect();
    let mut work: Vec<Paint> = Vec::new();
    let stamp = view.find_ref().stamp;
    let want_reveal = view.find_ref().reveal && !revealing;
    let below = view.composer_inset() + COMPOSER_CLEARANCE;
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
                        _ = b.reveal_range_within(range, BAR_CLEARANCE, below, cx);
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

/// The marks on a plain part of row `key` as it draws `text` (a call's label, command or output, a work row, a
/// subagent's line), for its `StyledText`: every match in the match ink and the current one in its own.
pub fn marks(view: &ThreadView, key: &str, part: &str, text: &str, cx: &App) -> Vec<(Range<usize>, HighlightStyle)> {
    let find = view.find_ref();
    let Some(needle) = find.needle.as_ref().filter(|_| find.open) else { return Vec::new() };
    if !find.hit_docs.contains_key(&(key.to_owned(), part.to_owned())) {
        return Vec::new();
    }
    let p = cx.palette();
    let current = find.current.and_then(|c| find.hit(c)).filter(|c| c.key == key && c.part == part).map(|c| c.ordinal);
    rendered_spans(text, needle)
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let ink = p.primary.opacity(if Some(i) == current { 0.45 } else { 0.2 });
            (r, HighlightStyle { background_color: Some(ink), ..HighlightStyle::default() })
        })
        .collect()
}

/// Escape closes the bar from anywhere inside it; `toggleChord`, Alt with C, W or R (Command and Option on the Mac, where
/// Option alone types a character), flips Match case, Match whole word or Use regular expression.
pub fn key_down(view: &mut ThreadView, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<ThreadView>) {
    if !view.find().open {
        return;
    }
    let keystroke = &event.keystroke;
    if keystroke.key == "escape" {
        cx.stop_propagation();
        close(view, window, cx);
        return;
    }
    let m = &keystroke.modifiers;
    if !m.alt || m.platform != cfg!(target_os = "macos") || m.control || m.shift {
        return;
    }
    let flip: fn(&mut Find) = match keystroke.key.as_str() {
        "c" => |f: &mut Find| f.options.match_case = !f.options.match_case,
        "w" => |f: &mut Find| f.options.whole_word = !f.options.whole_word,
        "r" => |f: &mut Find| f.options.regex = !f.options.regex,
        _ => return,
    };
    cx.stop_propagation();
    set_option(view, flip, cx);
}

/// `dropdown-glass popover-shadow`: the popover ground, at 96% under an 8 px blur off the Mac.
fn glass(el: gpui_kit::Div, p: &Palette) -> gpui_kit::Div {
    let el = el.border_1().border_color(p.popover_edge).shadow(p.popover_shadow.clone());
    if cfg!(target_os = "macos") {
        el.bg(p.popover)
    } else {
        el.bg(p.popover.opacity(0.96)).backdrop_blur(px(8.))
    }
}

/// `FIND_WORDS`' words for the bar's three buttons.
const OLDER: &str = "Older match";
const NEWER: &str = "Newer match";
const CLOSE: &str = "Close";

/// `Tip`: the word, then its chord in mono and muted 8 px after it. Today's app reads each chord with no context, so
/// Older and Newer name Enter and Shift+Enter alone.
fn tip(label: &'static str, chord: Option<String>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let (chord, mono, muted) = (chord.clone(), cx.theme().mono_font_family.clone(), cx.palette().muted_foreground);
        let content = move |_: &mut App| {
            h_flex().child(label).when_some(chord.clone(), |el, chord| {
                let named = chord.clone();
                el.child(
                    div()
                        .debug_selector(move || format!("find-chord:{named}"))
                        .ml(px(8.))
                        .font_family(mono.clone())
                        .text_color(muted)
                        .child(chord),
                )
            })
        };
        crate::ui::tooltip_view(content, window, cx)
    }
}

const MAC: bool = cfg!(target_os = "macos");

/// `toggleChord`: Option with Command on the Mac, where Option alone types a character, and Alt elsewhere.
fn toggle_chord(key: &str) -> String {
    chord_label(MAC, &format!("alt-{}{key}", if MAC { "cmd-" } else { "" }))
}

/// A chord as `formatShortcutLabel` writes it on the Mac or off it.
fn chord_label(mac: bool, chord: &str) -> String {
    gpui_kit::Keystroke::parse(chord).map_or_else(|_| chord.to_owned(), |k| crate::ui::context_menu::keys_label(&k, mac))
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
    let status = find.status().filter(|_| view.loaded());
    let can_step = !find.hits.is_empty();
    let pressed = [find.options.match_case, find.options.whole_word, find.options.regex, find.tools];
    let toggles = [
        (Glyph::CaseSensitive, "Match case", Some("c")),
        (Glyph::WholeWord, "Match whole word", Some("w")),
        (Glyph::Regex, "Use regular expression", Some("r")),
        (Glyph::Wrench, "Include tool calls", None),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (glyph, label, key))| {
        let on = pressed[i];
        div()
            .id(SharedString::from(format!("find-toggle-{i}")))
            .debug_selector(move || format!("find-toggle-{i}"))
            .tooltip(tip(label, key.map(toggle_chord)))
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
    let narrow = window.viewport_size().width < NARROW;
    let (inline, own_row) = if narrow { (None, Some(toggles)) } else { (Some(toggles), None) };
    let field = Input::new(&input)
        .appearance(false)
        .with_size(gpui_kit::component::Size::Small)
        .when_some(inline, |field, toggles| field.suffix(h_flex().gap_0().mr(px(-16.)).children(toggles)));
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
                .w(if narrow { px(72.) } else { px(96.) })
                .flex_none()
                .justify_end()
                .gap(px(6.))
                .whitespace_nowrap()
                .text_size(px(13.))
                .text_color(p.muted_foreground)
                .children(status),
        )
        .child(
            div().id("find-older-tip").debug_selector(|| "find-older".into()).tooltip(tip(OLDER, Some(chord_label(MAC, "enter")))).child(
                Button::icon_xs("find-older", Glyph::ChevronUp)
                    .held(!can_step)
                    .on_click(cx.listener(|view, _, window, cx| step(view, -1, window, cx))),
            ),
        )
        .child(
            div()
                .id("find-newer-tip")
                .debug_selector(|| "find-newer".into())
                .tooltip(tip(NEWER, Some(chord_label(MAC, "shift-enter"))))
                .child(
                    Button::icon_xs("find-newer", Glyph::ChevronDown)
                        .held(!can_step)
                        .on_click(cx.listener(|view, _, window, cx| step(view, 1, window, cx))),
                ),
        )
        .child(
            div()
                .id("find-close-tip")
                .debug_selector(|| "find-close".into())
                .tooltip(tip(CLOSE, Some(chord_label(MAC, "escape"))))
                .child(Button::icon_xs("find-close", Glyph::Close).on_click(cx.listener(|view, _, window, cx| close(view, window, cx)))),
        );
    // `FIND_WORDS.slow` has no counterpart: Rust's regex runs in time linear in the text, so no pattern runs long.
    let note = find
        .note()
        .map(|said| div().px(px(12.)).pb(px(8.)).text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(said));
    Some(
        h_flex()
            .absolute()
            .top(BAR_INSET)
            .left(BAR_INSET)
            .right(BAR_INSET)
            .justify_end()
            .child(
                glass(v_flex(), &p)
                    .id("find-bar")
                    .debug_selector(|| "find-bar".into())
                    .w_full()
                    .max_w(BAR_WIDTH)
                    .rounded(BAR_RADIUS)
                    .font_weight(FontWeight::NORMAL)
                    .occlude()
                    .capture_key_down(cx.listener(key_down))
                    .child(row)
                    .children(own_row.map(|toggles| h_flex().gap(px(4.)).px(px(4.)).pb(px(4.)).children(toggles)))
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
    fn the_note_says_a_broken_pattern_then_a_trimmed_thread_and_the_count_waits_on_older_pages() {
        let mut find = Find { query: "x".to_owned(), trimmed: true, ..Find::default() };
        assert_eq!(find.note(), Some(TRIMMED));
        find.invalid = true;
        assert_eq!(find.note(), Some(INVALID));
        find.invalid = false;
        find.partial = true;
        assert_eq!(find.status(), None, "no No results while older pages come in");
        find.docs = vec![Doc {
            key: "k".to_owned(),
            part: String::new(),
            turn: None,
            group: None,
            call: None,
            opens: None,
            tool: false,
            segments: vec!["x".to_owned()],
        }];
        find.hits = vec![(0, 0)];
        find.current = Some(0);
        assert_eq!(find.status().as_deref(), Some("1 of 1+"));
    }

    #[test]
    fn a_chord_reads_as_format_shortcut_label_writes_it_on_each_side() {
        assert_eq!(chord_label(true, "alt-cmd-c"), "⌥⌘C");
        assert_eq!(chord_label(true, "shift-enter"), "⇧Enter");
        assert_eq!(chord_label(false, "alt-c"), "Alt+C");
        assert_eq!(chord_label(false, "shift-enter"), "Shift+Enter");
        assert_eq!(chord_label(false, "escape"), "Esc");
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
