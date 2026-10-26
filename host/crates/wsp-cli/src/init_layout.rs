// SPDX-License-Identifier: AGPL-3.0-only
//! The few rules every wsp init screen shares: cut to a width with an ellipsis, pad cells into columns, wrap, the
//! focus bar, the help line, a card in the frame, and the confirm and password prompts drawn with those rules. Port of
//! `packages/host/src/init-layout.ts` at 65f6b89c7 over `@clack/core` 1.4.3 and `@clack/prompts` 1.7.0; the spec is
//! `spec/layout.md`.
use std::sync::{Arc, OnceLock};

use futures::future::BoxFuture;
use regex::Regex;
use tokio::sync::mpsc::UnboundedReceiver;
use unicode_segmentation::UnicodeSegmentation;

use wsp_words::js::{js_trim_end as trim_end_js, js_trim_start as trim_start_js, utf16::len as utf16};

use crate::hosts::Env;

/// init-layout.ts:13.
pub const GUTTER: &str = "  ";

/// clack's `isUnicodeSupported()` off Windows: `TERM` is not `linux`.
pub fn unicode_in(env: &Env) -> bool {
    env.get("TERM").map(String::as_str) != Some("linux")
}

/// The glyphs a screen draws, unicode or their ASCII stand-ins (init-layout.ts:15-16; @clack/prompts index.mjs:27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyphs {
    pub bar: &'static str,
    pub bar_focus: &'static str,
    pub bar_focus_end: &'static str,
    pub step_active: &'static str,
    pub step_cancel: &'static str,
    pub step_submit: &'static str,
    pub radio_active: &'static str,
    pub radio_inactive: &'static str,
    pub mask: &'static str,
}

/// The glyphs for a terminal that does or does not draw unicode.
pub fn glyphs(unicode: bool) -> Glyphs {
    if unicode {
        Glyphs {
            bar: "│",
            bar_focus: "┃",
            bar_focus_end: "┗",
            step_active: "◆",
            step_cancel: "■",
            step_submit: "◇",
            radio_active: "●",
            radio_inactive: "○",
            mask: "▪",
        }
    } else {
        Glyphs {
            bar: "|",
            bar_focus: "|",
            bar_focus_end: "+",
            step_active: "*",
            step_cancel: "x",
            step_submit: "o",
            radio_active: ">",
            radio_inactive: " ",
            mask: "*",
        }
    }
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F | 0x1F900..=0x1F9FF | 0x20000..=0x3FFFD)
}

fn emoji_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\p{Emoji_Presentation}|\x{FE0F}").expect("the emoji pattern compiles"))
}

fn mark_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\p{Mn}\p{Me}\p{Cf}]").expect("the mark pattern compiles"))
}

fn ansi_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"[\x1b\x{9b}][\[\]()#;?]*(?:(?:(?:(?:;[-a-zA-Z0-9/#&.:=?%@~_]+)*|[a-zA-Z0-9]+(?:;[-a-zA-Z0-9/#&.:=?%@~_]*)*)?(?:\x07|\x1b\\|\x{9c}))|(?:(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PR-TZcf-nq-uy=><~]))",
        )
        .expect("the ansi pattern compiles")
    })
}

/// node's `stripVTControlCharacters` (lib/internal/util/inspect.js `ansi`).
pub(crate) fn strip_vt(s: &str) -> String {
    ansi_re().replace_all(s, "").into_owned()
}

fn grapheme_cells(g: &str) -> usize {
    let first = g.chars().next().unwrap_or(' ');
    if is_wide(first) || emoji_re().is_match(g) {
        2
    } else if mark_re().is_match(g) {
        0
    } else {
        1
    }
}

/// The cells a terminal gives text: a wide character or an emoji two, a combining mark none.
pub fn cell_width(text: &str) -> usize {
    text.graphemes(true).map(grapheme_cells).sum()
}

/// init-layout.ts:32-44. `width` may be zero or negative: `""`.
pub fn ellipsize(text: &str, width: i64) -> String {
    if width <= 0 {
        return String::new();
    }
    if cell_width(text) as i64 <= width {
        return text.to_string();
    }
    let mut kept = String::new();
    let mut used = 0i64;
    for g in text.graphemes(true) {
        let n = grapheme_cells(g) as i64;
        if used + n > width - 1 {
            break;
        }
        kept.push_str(g);
        used += n;
    }
    format!("{kept}…")
}

/// init-layout.ts:52.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Right,
}

/// init-layout.ts:47-50.
pub fn pad_cells(text: &str, width: usize, align: Align) -> String {
    let pad = " ".repeat(width.saturating_sub(cell_width(text)));
    match align {
        Align::Right => format!("{pad}{text}"),
        Align::Left => format!("{text}{pad}"),
    }
}

/// init-layout.ts:55-62. Widths are UTF-16 lengths (`cell.length`), as `padStart` and `padEnd` count.
pub fn table(rows: &[Vec<String>], align: &[Align]) -> Vec<String> {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if widths.len() <= i {
                widths.resize(i + 1, 0);
            }
            widths[i] = widths[i].max(utf16(cell));
        }
    }
    rows.iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let pad = " ".repeat(widths[i].saturating_sub(utf16(cell)));
                    if align.get(i) == Some(&Align::Right) {
                        format!("{pad}{cell}")
                    } else {
                        format!("{cell}{pad}")
                    }
                })
                .collect();
            trim_end_js(&cells.join(GUTTER)).to_string()
        })
        .collect()
}

/// init-layout.ts:65-68: the stream's columns, else 80, capped at `cap` (100 by default).
pub fn width_of(columns: Option<u16>, cap: u16) -> u16 {
    columns.unwrap_or(80).min(cap)
}

/// init-layout.ts:71-73: the stream's rows, else 20.
pub fn rows_of(rows: Option<u16>) -> u16 {
    rows.unwrap_or(20)
}

/// A window over a list (init-layout.ts:76-80): `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
}

/// init-layout.ts:76-80.
pub fn viewport(total: usize, cursor: usize, height: i64) -> Window {
    let h = height.max(1);
    let (total, cursor) = (total as i64, cursor as i64);
    let start = (cursor - h / 2).min(total - h).max(0);
    let end = total.min(start + h);
    Window { start: start as usize, end: end as usize }
}

/// init-layout.ts:83-90 (`named` is 3 by default). Lengths are UTF-16 lengths.
pub fn summarize(labels: &[String], width: i64, named: usize) -> String {
    let more = |n: usize| if labels.len() > n { format!(" +{} more", labels.len() - n) } else { String::new() };
    let mut n = named.min(labels.len());
    while n > 1 {
        let line = format!("{}{}", labels[..n].join(", "), more(n));
        if utf16(&line) as i64 <= width {
            return line;
        }
        n -= 1;
    }
    if labels.is_empty() {
        return String::new();
    }
    format!("{}{}", ellipsize(&labels[0], width - more(1).len() as i64), more(1))
}

/// init-layout.ts:94-101 (`indent` is two spaces by default). Indexes and lengths are UTF-16, as the TS counts them.
pub fn wrap(text: &str, width: usize, indent: &str) -> Vec<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= width {
        return vec![text.to_string()];
    }
    let lead = units.len() - utf16(trim_start_js(text));
    let space = |u: &u16| *u == b' ' as u16;
    let mut cut = units[..=width].iter().rposition(space).map_or(-1i64, |i| i as i64);
    if cut < lead as i64 {
        cut = units.iter().skip(lead).position(space).map_or(-1, |i| (i + lead) as i64);
    }
    if cut < 0 {
        return vec![ellipsize(text, width as i64)];
    }
    let cut = cut as usize;
    let head = String::from_utf16_lossy(&units[..cut]);
    let rest = String::from_utf16_lossy(&units[cut + 1..]);
    let mut out = vec![ellipsize(trim_end_js(&head), width as i64)];
    out.extend(wrap(&format!("{indent}{}", trim_start_js(&rest)), width, indent));
    out
}

/// init-layout.ts:104: the UTF-16 length of the row with its escape codes taken out (`stripVTControlCharacters`).
pub fn cells(row: &str) -> usize {
    utf16(&strip_vt(row))
}

/// init-layout.ts:110-113.
pub fn rewind(drawn: &[usize], columns: u16) -> String {
    let columns = columns.max(1) as usize;
    let rows: usize = drawn.iter().map(|w| w.div_ceil(columns).max(1)).sum();
    if rows > 0 {
        format!("\x1b[{rows}A\x1b[G\x1b[J")
    } else {
        String::new()
    }
}

const COLOUR_TERMS: &[(&str, u8)] = &[
    ("eterm", 4),
    ("cons25", 4),
    ("console", 4),
    ("cygwin", 4),
    ("dtterm", 4),
    ("gnome", 4),
    ("hurd", 4),
    ("jfbterm", 4),
    ("konsole", 4),
    ("kterm", 4),
    ("mlterm", 4),
    ("mosh", 24),
    ("putty", 4),
    ("st", 4),
    ("rxvt-unicode-24bit", 24),
    ("terminator", 24),
    ("xterm-kitty", 24),
];

const CI_COLOURS: &[(&str, u8)] = &[
    ("APPVEYOR", 8),
    ("BUILDKITE", 8),
    ("CIRCLECI", 24),
    ("DRONE", 8),
    ("GITEA_ACTIONS", 24),
    ("GITHUB_ACTIONS", 24),
    ("GITLAB_CI", 8),
    ("TRAVIS", 8),
];

fn term_reg_matches(term: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"ansi|color|linux|direct|^con[0-9]*x[0-9]|^rxvt|^screen|^xterm|^vt100|^vt220").expect("the term pattern compiles")
    })
    .is_match(term)
}

/// node 22's `WriteStream.prototype.getColorDepth(env)` (lib/internal/tty.js): 1, 4, 8 or 24.
pub fn node_color_depth(env: &Env) -> u8 {
    let get = |k: &str| env.get(k).map(String::as_str);
    let set = |k: &str| get(k).is_some_and(|v| !v.is_empty());
    if let Some(force) = get("FORCE_COLOR") {
        return match force {
            "" | "1" | "true" => 4,
            "2" => 8,
            "3" => 24,
            _ => 1,
        };
    }
    if env.contains_key("NODE_DISABLE_COLORS") || env.contains_key("NO_COLOR") || get("TERM") == Some("dumb") {
        return 1;
    }
    if set("TMUX") {
        return 24;
    }
    if env.contains_key("TF_BUILD") && env.contains_key("AGENT_NAME") {
        return 4;
    }
    if env.contains_key("CI") {
        if let Some((_, colours)) = CI_COLOURS.iter().find(|(name, _)| env.contains_key(*name)) {
            return *colours;
        }
        return if get("CI_NAME") == Some("codeship") { 8 } else { 1 };
    }
    if let Some(version) = get("TEAMCITY_VERSION") {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"^(9\.(0*[1-9][0-9]*)\.|[0-9]{2,}\.)").expect("the teamcity pattern compiles"));
        return if re.is_match(version) { 4 } else { 1 };
    }
    match get("TERM_PROGRAM") {
        Some("iTerm.app") => {
            let old = !set("TERM_PROGRAM_VERSION")
                || get("TERM_PROGRAM_VERSION").is_some_and(|v| v.starts_with("0.") || v.starts_with("1.") || v.starts_with("2."));
            return if old { 8 } else { 24 };
        }
        Some("HyperTerm") | Some("MacTerm") => return 24,
        Some("Apple_Terminal") => return 8,
        _ => {}
    }
    if matches!(get("COLORTERM"), Some("truecolor") | Some("24bit")) {
        return 24;
    }
    if let Some(term) = get("TERM").filter(|t| !t.is_empty()) {
        if term.contains("truecolor") {
            return 24;
        }
        if term.starts_with("xterm-256") {
            return 8;
        }
        let lower = term.to_lowercase();
        if let Some((_, colours)) = COLOUR_TERMS.iter().find(|(name, _)| *name == lower) {
            return *colours;
        }
        if term_reg_matches(&lower) {
            return 4;
        }
    }
    if set("COLORTERM") {
        return 4;
    }
    1
}

/// init-layout.ts:119-122.
pub fn colour_depth(tty: bool, env: &Env) -> u8 {
    if !tty && !env.contains_key("FORCE_COLOR") {
        return 1;
    }
    node_color_depth(env)
}

/// Whether node's `styleText` paints, for the process's own stdout (node 22 `util.styleText` with `validateStream`):
/// `FORCE_COLOR` set means `node_color_depth(env) > 2`; else stdout is a terminal and `node_color_depth(env) > 2`.
pub fn styles_on(env: &Env, stdout_tty: bool) -> bool {
    if env.contains_key("FORCE_COLOR") {
        return node_color_depth(env) > 2;
    }
    stdout_tty && node_color_depth(env) > 2
}

fn style_codes(format: &str) -> Option<(u8, u8)> {
    Some(match format {
        "dim" => (2, 22),
        "bold" => (1, 22),
        "strikethrough" => (9, 29),
        "cyan" => (36, 39),
        "green" => (32, 39),
        "red" => (31, 39),
        "gray" => (90, 39),
        "inverse" => (7, 27),
        "hidden" => (8, 28),
        _ => return None,
    })
}

/// node's `styleText(formats, text)` when `on`: each format's open code in order, the text, the close codes in the
/// reverse order (`dim` 2/22, `bold` 1/22, `strikethrough` 9/29, `cyan` 36/39, `green` 32/39, `red` 31/39, `gray`
/// 90/39, `inverse` 7/27, `hidden` 8/28). When not `on`, the text alone.
pub fn style_text(formats: &[&str], text: &str, on: bool) -> String {
    if !on {
        return text.to_string();
    }
    let codes: Vec<(u8, u8)> = formats.iter().filter_map(|f| style_codes(f)).collect();
    let open: String = codes.iter().map(|(o, _)| format!("\x1b[{o}m")).collect();
    let close: String = codes.iter().rev().map(|(_, c)| format!("\x1b[{c}m")).collect();
    format!("{open}{text}{close}")
}

/// init-layout.ts:125.
pub fn grey(n: u8, s: &str) -> String {
    format!("\x1b[38;5;{n}m{s}\x1b[39m")
}

/// init-layout.ts:128.
pub const GREY_BRIGHT: u8 = 247;
pub const GREY_MID: u8 = 243;
pub const GREY_DIM: u8 = 239;

/// init-layout.ts:131: `styleText("cyan", s)`.
pub fn accent(s: &str, on: bool) -> String {
    style_text(&["cyan"], s, on)
}

/// init-layout.ts:135-137.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpKey {
    pub key: String,
    pub does: String,
}

/// init-layout.ts:141: plain at depth 1 or under, `styleText("dim")` under 8 (painted only where `on`), grey 243 from 8.
pub fn muted(s: &str, depth: u8, on: bool) -> String {
    if depth <= 1 {
        s.to_string()
    } else if depth < 8 {
        style_text(&["dim"], s, on)
    } else {
        grey(GREY_MID, s)
    }
}

/// init-layout.ts:144-148.
pub fn help_line(keys: &[HelpKey], depth: u8, on: bool) -> String {
    if depth <= 1 {
        return keys.iter().map(|k| format!("{} {}", k.key, k.does)).collect::<Vec<_>>().join("   ");
    }
    let key_of = |s: &str| if depth < 8 { s.to_string() } else { grey(GREY_BRIGHT, s) };
    let gap = muted("   ", depth, on);
    keys.iter().map(|k| format!("{} {}", key_of(&k.key), muted(&k.does, depth, on))).collect::<Vec<_>>().join(&gap)
}

/// init-layout.ts:152.
pub const CARD_FRAME: u16 = 3;

/// A stream a screen writes to: raw text, and what it knows of the terminal under it.
pub trait ScreenOut: Send + Sync {
    fn write(&self, text: &str);
    fn columns(&self) -> Option<u16>;
    fn rows(&self) -> Option<u16>;
    fn is_tty(&self) -> bool;
}

/// What a screen reads off the process it runs in: whether styleText paints (`styles_on`), the help line's colour
/// depth (`colour_depth(output.is_tty(), env)`), the glyph set (`unicode_in`), and the process stdout's columns that
/// clack hard-wraps a frame at (`None` off a terminal: no wrap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    pub on: bool,
    pub depth: u8,
    pub unicode: bool,
    pub stdout_columns: Option<u16>,
}

/// init-layout.ts:156-159 over clack's `log.message` (@clack/prompts index.mjs:726-748): one write of the bar line,
/// the bold title on the green submit glyph, and each line down the gray bar, a line wider than
/// `width_of(columns, 100) - 3` once its codes are out wrapped by `wrap`.
pub fn card(title: &str, lines: &[String], output: &dyn ScreenOut, paint: &Paint) {
    let g = glyphs(paint.unicode);
    let width = width_of(output.columns(), 100).saturating_sub(CARD_FRAME) as usize;
    let gray_bar = style_text(&["gray"], g.bar, paint.on);
    let mut out = vec![gray_bar.clone()];
    let symbol = style_text(&["green"], g.step_submit, paint.on);
    if title.is_empty() {
        out.push(symbol);
    } else {
        out.push(format!("{symbol}  {}", style_text(&["bold"], title, paint.on)));
    }
    for line in lines {
        let parts = if utf16(&strip_vt(line)) <= width { vec![line.clone()] } else { wrap(line, width, "  ") };
        for p in parts {
            out.push(if p.is_empty() { gray_bar.clone() } else { format!("{gray_bar}  {p}") });
        }
    }
    output.write(&format!("{}\n", out.join("\n")));
}

/// What a prompt came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer<T> {
    Value(T),
    Cancel,
}

/// init-layout.ts:167-178. `input` carries the bytes the terminal sent, one chunk per read.
pub struct PromptOptions {
    pub message: String,
    pub hint: Option<String>,
    pub input: UnboundedReceiver<String>,
    pub output: Arc<dyn ScreenOut>,
    pub paint: Paint,
    /// Ends the prompt from outside, as a cancel.
    pub signal: Option<BoxFuture<'static, ()>>,
}

const SHOW: &str = "\x1b[?25h";
const HIDE: &str = "\x1b[?25l";
const ERASE_DOWN: &str = "\x1b[J";
const ERASE_LINE: &str = "\x1b[2K\x1b[G";
const CONFIRM_KEYS: &[(&str, &str)] = &[("← →", "change"), ("y n", "answer"), ("enter", "choose"), ("esc", "cancel")];
const ENTER_KEYS: &[(&str, &str)] = &[("enter", "next"), ("esc", "cancel")];
const ACTIONS: &[&str] = &["up", "down", "left", "right", "space", "enter", "cancel"];

/// sisteransi's `cursor.move(x, y)`.
fn move_cursor(x: i64, y: i64) -> String {
    let mut out = String::new();
    if x < 0 {
        out.push_str(&format!("\x1b[{}D", -x));
    } else if x > 0 {
        out.push_str(&format!("\x1b[{x}C"));
    }
    if y < 0 {
        out.push_str(&format!("\x1b[{}A", -y));
    } else if y > 0 {
        out.push_str(&format!("\x1b[{y}B"));
    }
    out
}

/// The SGR code open after an escape whose code (`\[(\d+)m`) starts at `chars[at]`.
fn sgr_code_at(chars: &[char], at: usize) -> Option<u32> {
    if chars.get(at) != Some(&'[') {
        return None;
    }
    let digits: String = chars[at + 1..].iter().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || chars.get(at + 1 + digits.len()) != Some(&'m') {
        return None;
    }
    digits.parse().ok()
}

/// fast-wrap-ansi's `getClosingCode`, for the codes a screen opens.
fn closing_code(code: u32) -> Option<u32> {
    match code {
        30..=37 | 90..=97 => Some(39),
        40..=47 | 100..=107 => Some(49),
        1 | 2 => Some(22),
        3 => Some(23),
        4 => Some(24),
        7 => Some(27),
        8 => Some(28),
        9 => Some(29),
        _ => None,
    }
}

fn char_cells(c: char) -> usize {
    if c.is_control() {
        0
    } else {
        cell_width(c.encode_utf8(&mut [0u8; 4]))
    }
}

fn string_cells(s: &str) -> usize {
    cell_width(&strip_vt(s))
}

fn wrap_word(rows: &mut Vec<String>, word: &str, columns: i64) {
    let chars: Vec<char> = word.chars().collect();
    let mut visible = rows.last().map_or(0, |r| string_cells(r) as i64);
    let mut inside_escape = false;
    for (i, &ch) in chars.iter().enumerate() {
        let cl = char_cells(ch) as i64;
        if visible + cl <= columns {
            rows.last_mut().expect("a row").push(ch);
        } else {
            rows.push(ch.to_string());
            visible = 0;
        }
        if ch == '\x1b' || ch == '\u{9b}' {
            inside_escape = true;
        }
        if inside_escape {
            if ch == 'm' {
                inside_escape = false;
            }
        } else {
            visible += cl;
            if visible == columns && i + 1 < chars.len() {
                rows.push(String::new());
                visible = 0;
            }
        }
    }
    let last_len = rows.last().map_or(0, String::len);
    if visible == 0 && last_len > 0 && rows.len() > 1 {
        let tail = rows.pop().expect("a row");
        rows.last_mut().expect("a row").push_str(&tail);
    }
}

fn wrap_line_hard(line: &str, columns: i64) -> String {
    let mut rows = vec![String::new()];
    let mut row_len = 0i64;
    for (index, word) in line.split(' ').enumerate() {
        if index != 0 {
            if row_len >= columns {
                rows.push(String::new());
                row_len = 0;
            }
            rows.last_mut().expect("a row").push(' ');
            row_len += 1;
        }
        let word_len = string_cells(word) as i64;
        if word_len > columns {
            let remaining = columns - row_len;
            let this_line = 1 + (word_len - remaining - 1).div_euclid(columns);
            let next_line = (word_len - 1).div_euclid(columns);
            if next_line < this_line {
                rows.push(String::new());
            }
            wrap_word(&mut rows, word, columns);
            row_len = rows.last().map_or(0, |r| string_cells(r) as i64);
            continue;
        }
        if row_len + word_len > columns && row_len != 0 && word_len != 0 {
            rows.push(String::new());
            row_len = 0;
        }
        rows.last_mut().expect("a row").push_str(word);
        row_len += word_len;
    }
    let pre: Vec<char> = rows.join("\n").chars().collect();
    let mut out = String::new();
    let mut open: Option<u32> = None;
    for (i, &ch) in pre.iter().enumerate() {
        out.push(ch);
        if ch == '\x1b' || ch == '\u{9b}' {
            match sgr_code_at(&pre, i + 1) {
                Some(39) | Some(0) => open = None,
                Some(code) => open = Some(code),
                None => {}
            }
        }
        if pre.get(i + 1) == Some(&'\n') {
            if let Some(close) = open.and_then(closing_code) {
                out.push_str(&format!("\x1b[{close}m"));
            }
        } else if ch == '\n' {
            if let Some(code) = open.filter(|c| closing_code(*c).is_some()) {
                out.push_str(&format!("\x1b[{code}m"));
            }
        }
    }
    out
}

/// clack's `wrapAnsi` at the process's stdout columns; `None` passes the frame as it is.
fn wrap_frame(frame: &str, columns: Option<u16>) -> String {
    match columns {
        None => frame.to_string(),
        Some(c) => frame.split('\n').map(|l| wrap_line_hard(l, (c as i64).max(1))).collect::<Vec<_>>().join("\n"),
    }
}

struct LineDiff {
    lines: Vec<usize>,
    before: usize,
    after: usize,
}

fn diff_lines(prev: &str, next: &str) -> Option<LineDiff> {
    if prev == next {
        return None;
    }
    let (e, t): (Vec<&str>, Vec<&str>) = (prev.split('\n').collect(), next.split('\n').collect());
    let n = e.len().max(t.len());
    let lines = (0..n).filter(|&i| e.get(i) != t.get(i)).collect();
    Some(LineDiff { lines, before: e.len(), after: t.len() })
}

/// The state the confirm and password prompts share (init-layout.ts:196-232).
enum Kind {
    Confirm { value: bool },
    Password { line: Vec<char>, cursor: usize },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Initial,
    Active,
    Submit,
    Cancel,
}

struct Key {
    seq: String,
    name: Option<String>,
    text: Option<char>,
}

fn key_name_of_csi(seq: &str) -> Option<&'static str> {
    Some(match seq {
        "\x1b[A" | "\x1bOA" => "up",
        "\x1b[B" | "\x1bOB" => "down",
        "\x1b[C" | "\x1bOC" => "right",
        "\x1b[D" | "\x1bOD" => "left",
        "\x1b[3~" => "delete",
        "\x1b[H" | "\x1bOH" | "\x1b[1~" => "home",
        "\x1b[F" | "\x1bOF" | "\x1b[4~" => "end",
        _ => return None,
    })
}

fn keys_of(chunk: &str) -> Vec<Key> {
    let cs: Vec<char> = chunk.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '\x1b' {
            if matches!(cs.get(i + 1), Some('[') | Some('O')) {
                let mut j = i + 2;
                while j < cs.len() && ('\x20'..='\x3f').contains(&cs[j]) {
                    j += 1;
                }
                if j < cs.len() && ('\x40'..='\x7e').contains(&cs[j]) {
                    let seq: String = cs[i..=j].iter().collect();
                    out.push(Key { name: key_name_of_csi(&seq).map(str::to_string), seq, text: None });
                    i = j + 1;
                    continue;
                }
            }
            if let Some(&next) = cs.get(i + 1) {
                let name = next.is_ascii_alphabetic().then(|| next.to_ascii_lowercase().to_string());
                out.push(Key { seq: format!("\x1b{next}"), name, text: None });
                i += 2;
            } else {
                out.push(Key { seq: "\x1b".into(), name: Some("escape".into()), text: None });
                i += 1;
            }
            continue;
        }
        i += 1;
        let key = match c {
            '\r' => Key { seq: c.to_string(), name: Some("return".into()), text: None },
            '\n' => Key { seq: c.to_string(), name: Some("enter".into()), text: None },
            '\t' => Key { seq: c.to_string(), name: Some("tab".into()), text: None },
            '\x7f' | '\x08' => Key { seq: c.to_string(), name: Some("backspace".into()), text: None },
            ' ' => Key { seq: c.to_string(), name: Some("space".into()), text: Some(c) },
            c if (c as u32) < 0x20 => {
                let letter = char::from((c as u8) + 96).to_string();
                Key { seq: c.to_string(), name: Some(letter), text: None }
            }
            c => {
                let name = c.is_ascii_alphabetic().then(|| c.to_ascii_lowercase().to_string());
                Key { seq: c.to_string(), name, text: Some(c) }
            }
        };
        out.push(key);
    }
    out
}

fn alias_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "k" => "up",
        "j" => "down",
        "h" => "left",
        "l" => "right",
        "escape" => "cancel",
        _ => return None,
    })
}

fn is_cancel(k: &Key) -> bool {
    k.seq == "\x03" || k.name.as_deref() == Some("escape")
}

struct Prompt {
    message: String,
    hint: Option<String>,
    output: Arc<dyn ScreenOut>,
    paint: Paint,
    phase: Phase,
    prev: String,
    kind: Kind,
    listening: bool,
}

impl Prompt {
    fn write(&self, text: &str) {
        self.output.write(text);
    }

    fn glyphs(&self) -> Glyphs {
        glyphs(self.paint.unicode)
    }

    fn open(&self) -> bool {
        !matches!(self.phase, Phase::Submit | Phase::Cancel)
    }

    fn paint_styled(&self, formats: &[&str], text: &str) -> String {
        style_text(formats, text, self.paint.on)
    }

    fn radio(&self, label: &str, on: bool) -> String {
        let g = self.glyphs();
        if on {
            format!("{} {label}", self.paint_styled(&["cyan"], g.radio_active))
        } else {
            format!("{} {}", self.paint_styled(&["dim"], g.radio_inactive), self.paint_styled(&["dim"], label))
        }
    }

    fn body(&self) -> Vec<String> {
        match &self.kind {
            Kind::Confirm { value } => {
                let on = *value;
                let picked = if on { "Yes" } else { "No" };
                match self.phase {
                    Phase::Submit => vec![self.paint_styled(&["dim"], picked)],
                    Phase::Cancel => vec![self.paint_styled(&["strikethrough", "dim"], picked)],
                    _ => {
                        let slash = self.paint_styled(&["dim"], "/");
                        vec![format!("{} {slash} {}", self.radio("Yes", on), self.radio("No", !on))]
                    }
                }
            }
            Kind::Password { line, cursor } => {
                let g = self.glyphs();
                let mask: Vec<char> = line.iter().flat_map(|c| std::iter::repeat_n(g.mask, c.len_utf16())).flat_map(str::chars).collect();
                let mask_text: String = mask.iter().collect();
                match self.phase {
                    Phase::Submit | Phase::Cancel if mask.is_empty() => Vec::new(),
                    Phase::Submit => vec![self.paint_styled(&["dim"], &mask_text)],
                    Phase::Cancel => vec![self.paint_styled(&["strikethrough", "dim"], &mask_text)],
                    _ => {
                        let at: usize = line[..*cursor].iter().map(|c| c.len_utf16()).sum();
                        let shown = if at >= mask.len() {
                            format!("{mask_text}{}", self.paint_styled(&["inverse", "hidden"], "_"))
                        } else {
                            let before: String = mask[..at].iter().collect();
                            let under = self.paint_styled(&["inverse"], &mask[at].to_string());
                            let after: String = mask[at + 1..].iter().collect();
                            format!("{before}{under}{after}")
                        };
                        vec![shown]
                    }
                }
            }
        }
    }

    fn frame(&self) -> String {
        let g = self.glyphs();
        let width = width_of(self.output.columns(), 100).saturating_sub(CARD_FRAME) as usize;
        let open = self.open();
        let bar = self.paint_styled(&["dim"], if open { g.bar_focus } else { g.bar });
        let glyph = match self.phase {
            Phase::Submit => self.paint_styled(&["green"], g.step_submit),
            Phase::Cancel => self.paint_styled(&["red"], g.step_cancel),
            _ => self.paint_styled(&["cyan"], g.step_active),
        };
        let mut lines: Vec<String> = Vec::new();
        for (i, l) in self.message.split('\n').flat_map(|m| wrap(m, width, "  ")).enumerate() {
            let head = if i == 0 { glyph.clone() } else { bar.clone() };
            let text = if open { self.paint_styled(&["cyan"], &l) } else { l };
            lines.push(format!("{head}  {text}"));
        }
        if let Some(hint) = &self.hint {
            for h in hint.split('\n') {
                for l in wrap(h, width, "  ") {
                    lines.push(format!("{bar}  {}", self.paint_styled(&["dim"], &l)));
                }
            }
        }
        for b in self.body() {
            lines.push(format!("{bar}  {b}"));
        }
        if open {
            let keys: Vec<HelpKey> = match &self.kind {
                Kind::Confirm { .. } => CONFIRM_KEYS,
                Kind::Password { .. } => ENTER_KEYS,
            }
            .iter()
            .map(|(k, d)| HelpKey { key: k.to_string(), does: d.to_string() })
            .collect();
            let end = self.paint_styled(&["dim"], g.bar_focus_end);
            lines.push(format!("{end}  {}", help_line(&keys, self.paint.depth, self.paint.on)));
        }
        lines.join("\n")
    }

    fn restore_cursor(&self) {
        let up = wrap_frame(&self.prev, self.paint.stdout_columns).split('\n').count() as i64 - 1;
        self.write(&move_cursor(-999, -up));
    }

    fn draw(&mut self) {
        let t = wrap_frame(&self.frame(), self.paint.stdout_columns);
        if t == self.prev {
            return;
        }
        if self.phase == Phase::Initial {
            self.write(HIDE);
        } else {
            let rows = rows_of(self.output.rows()) as i64;
            let diff = diff_lines(&self.prev, &t);
            self.restore_cursor();
            if let Some(d) = diff {
                let n = (d.after as i64 - rows).max(0) as usize;
                let s = (d.before as i64 - rows).max(0) as usize;
                let Some(mut r) = d.lines.iter().copied().find(|&o| o >= n) else {
                    self.prev = t;
                    return;
                };
                let rows_t: Vec<&str> = t.split('\n').collect();
                if d.lines.len() == 1 {
                    self.write(&move_cursor(0, r as i64 - s as i64));
                    self.write(ERASE_LINE);
                    self.write(rows_t.get(r).copied().unwrap_or("undefined"));
                    self.prev = t.clone();
                    self.write(&move_cursor(0, rows_t.len() as i64 - r as i64 - 1));
                    return;
                }
                if d.lines.len() > 1 {
                    if n < s {
                        r = n;
                    } else {
                        let h = r as i64 - s as i64;
                        if h > 0 {
                            self.write(&move_cursor(0, h));
                        }
                    }
                    self.write(ERASE_DOWN);
                    self.write(&rows_t[r.min(rows_t.len())..].join("\n"));
                    self.prev = t;
                    return;
                }
            }
            self.write(ERASE_DOWN);
        }
        self.write(&t);
        if self.phase == Phase::Initial {
            self.phase = Phase::Active;
        }
        self.prev = t;
    }

    fn close(&mut self) {
        self.write("\n");
        if self.listening {
            self.write(SHOW);
            self.listening = false;
        }
    }

    fn finish_key(&mut self) {
        self.draw();
        if matches!(self.phase, Phase::Submit | Phase::Cancel) {
            self.close();
        }
    }

    fn cursor_action(&mut self, action: &str) {
        if let Kind::Confirm { value } = &mut self.kind {
            if action != "cancel" {
                *value = !*value;
            }
        }
    }

    fn key(&mut self, k: &Key) {
        if !self.listening {
            return;
        }
        match self.kind {
            Kind::Confirm { .. } => self.confirm_key(k),
            Kind::Password { .. } => self.password_key(k),
        }
    }

    fn confirm_key(&mut self, k: &Key) {
        if let Some(name) = k.name.as_deref() {
            if let Some(action) = alias_of(name) {
                self.cursor_action(action);
            }
            if ACTIONS.contains(&name) {
                self.cursor_action(name);
            }
        }
        let t = k.seq.to_lowercase();
        if t == "y" || t == "n" {
            self.write("\x1b[1A");
            if let Kind::Confirm { value } = &mut self.kind {
                *value = t == "y";
            }
            self.phase = Phase::Submit;
            self.close();
        }
        if k.name.as_deref() == Some("return") && self.phase != Phase::Cancel {
            self.phase = Phase::Submit;
        }
        if is_cancel(k) {
            self.phase = Phase::Cancel;
        }
        self.finish_key();
    }

    fn password_key(&mut self, k: &Key) {
        let is_return = k.name.as_deref() == Some("return");
        if !is_return {
            if let Kind::Password { line, cursor } = &mut self.kind {
                match k.name.as_deref() {
                    Some("left") => *cursor = cursor.saturating_sub(1),
                    Some("right") => *cursor = (*cursor + 1).min(line.len()),
                    Some("home") => *cursor = 0,
                    Some("end") => *cursor = line.len(),
                    Some("backspace") if *cursor > 0 => {
                        *cursor -= 1;
                        line.remove(*cursor);
                    }
                    Some("delete") if *cursor < line.len() => {
                        line.remove(*cursor);
                    }
                    _ => {
                        if let Some(c) = k.text {
                            line.insert(*cursor, c);
                            *cursor += 1;
                        }
                    }
                }
            }
        }
        if is_return {
            self.phase = Phase::Submit;
        }
        if is_cancel(k) {
            self.phase = Phase::Cancel;
        }
        self.finish_key();
    }
}

async fn wait_signal(signal: &mut Option<BoxFuture<'static, ()>>) {
    match signal {
        Some(f) => f.await,
        None => std::future::pending().await,
    }
}

/// Runs one prompt to its end: a key answers it, the signal or the end of input cancels it.
async fn run(o: PromptOptions, kind: Kind) -> Prompt {
    let PromptOptions { message, hint, input, output, paint, signal } = o;
    let mut input = input;
    let mut signal = signal;
    let mut p = Prompt { message, hint, output, paint, phase: Phase::Initial, prev: String::new(), kind, listening: true };
    p.draw();
    loop {
        tokio::select! {
            _ = wait_signal(&mut signal) => {
                p.phase = Phase::Cancel;
                p.close();
                return p;
            }
            chunk = input.recv() => match chunk {
                Some(text) => {
                    for k in keys_of(&text) {
                        p.key(&k);
                        if !p.listening {
                            break;
                        }
                    }
                    if !p.listening {
                        return p;
                    }
                }
                None => {
                    p.phase = Phase::Cancel;
                    p.close();
                    return p;
                }
            },
        }
    }
}

/// init-layout.ts:196-218: marker on No unless `initial` is true; left/right move it, `y` and `n` answer at once,
/// enter takes the marked answer, a lone escape and ctrl-c cancel.
pub async fn confirm_prompt(o: PromptOptions, initial: Option<bool>) -> Answer<bool> {
    let p = run(o, Kind::Confirm { value: initial.unwrap_or(false) }).await;
    match (p.phase, p.kind) {
        (Phase::Submit, Kind::Confirm { value }) => Answer::Value(value),
        _ => Answer::Cancel,
    }
}

/// init-layout.ts:221-232: every character drawn as the mask, the text never written; enter on nothing is `""`.
pub async fn password_prompt(o: PromptOptions) -> Answer<String> {
    let p = run(o, Kind::Password { line: Vec::new(), cursor: 0 }).await;
    match (p.phase, p.kind) {
        (Phase::Submit, Kind::Password { line, .. }) => Answer::Value(line.into_iter().collect()),
        _ => Answer::Cancel,
    }
}

/// Whether the stream is a terminal (init-layout.ts:116); `None` is no stream.
pub fn is_tty(output: Option<&dyn ScreenOut>) -> bool {
    output.is_some_and(|o| o.is_tty())
}
