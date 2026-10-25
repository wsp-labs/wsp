// SPDX-License-Identifier: AGPL-3.0-only
//! The few rules every wsp init screen shares: cut to a width with an ellipsis, pad cells into columns, wrap, the
//! focus bar, the help line, a card in the frame, and the confirm and password prompts drawn with those rules. Port of
//! `packages/host/src/init-layout.ts` at 65f6b89c7 over `@clack/core` 1.4.3 and `@clack/prompts` 1.7.0; the spec is
//! `spec/layout.md`.
use std::sync::Arc;

use crate::hosts::Env;

/// init-layout.ts:13.
pub const GUTTER: &str = "  ";

/// clack's `isUnicodeSupported()` off Windows: `TERM` is not `linux`.
pub fn unicode_in(env: &Env) -> bool {
    todo!()
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
    todo!()
}

/// init-layout.ts:24-28: grapheme clusters (Intl.Segmenter's default granularity), each two cells when wide or an
/// emoji, none when it opens with a mark or a format character, else one.
pub fn cell_width(text: &str) -> usize {
    todo!()
}

/// init-layout.ts:32-44. `width` may be zero or negative: `""`.
pub fn ellipsize(text: &str, width: i64) -> String {
    todo!()
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
    todo!()
}

/// init-layout.ts:55-62. Widths are UTF-16 lengths (`cell.length`), as `padStart` and `padEnd` count.
pub fn table(rows: &[Vec<String>], align: &[Align]) -> Vec<String> {
    todo!()
}

/// init-layout.ts:65-68: the stream's columns, else 80, capped at `cap` (100 by default).
pub fn width_of(columns: Option<u16>, cap: u16) -> u16 {
    todo!()
}

/// init-layout.ts:71-73: the stream's rows, else 20.
pub fn rows_of(rows: Option<u16>) -> u16 {
    todo!()
}

/// A window over a list (init-layout.ts:76-80): `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
}

/// init-layout.ts:76-80.
pub fn viewport(total: usize, cursor: usize, height: i64) -> Window {
    todo!()
}

/// init-layout.ts:83-90 (`named` is 3 by default). Lengths are UTF-16 lengths.
pub fn summarize(labels: &[String], width: i64, named: usize) -> String {
    todo!()
}

/// init-layout.ts:94-101 (`indent` is two spaces by default). Indexes and lengths are UTF-16, as the TS counts them.
pub fn wrap(text: &str, width: usize, indent: &str) -> Vec<String> {
    todo!()
}

/// init-layout.ts:104: the UTF-16 length of the row with its escape codes taken out (`stripVTControlCharacters`).
pub fn cells(row: &str) -> usize {
    todo!()
}

/// init-layout.ts:110-113.
pub fn rewind(drawn: &[usize], columns: u16) -> String {
    todo!()
}

/// node 22's `WriteStream.prototype.getColorDepth(env)` (lib/internal/tty.js): 1, 4, 8 or 24.
pub fn node_color_depth(env: &Env) -> u8 {
    todo!()
}

/// init-layout.ts:119-122.
pub fn colour_depth(tty: bool, env: &Env) -> u8 {
    todo!()
}

/// Whether node's `styleText` paints, for the process's own stdout (node 22 `util.styleText` with `validateStream`):
/// `FORCE_COLOR` set means `node_color_depth(env) > 2`; else stdout is a terminal and `node_color_depth(env) > 2`.
pub fn styles_on(env: &Env, stdout_tty: bool) -> bool {
    todo!()
}

/// node's `styleText(formats, text)` when `on`: each format's open code in order, the text, the close codes in the
/// reverse order (`dim` 2/22, `bold` 1/22, `strikethrough` 9/29, `cyan` 36/39, `green` 32/39, `red` 31/39, `gray`
/// 90/39, `inverse` 7/27, `hidden` 8/28). When not `on`, the text alone.
pub fn style_text(formats: &[&str], text: &str, on: bool) -> String {
    todo!()
}

/// init-layout.ts:125.
pub fn grey(n: u8, s: &str) -> String {
    todo!()
}

/// init-layout.ts:128.
pub const GREY_BRIGHT: u8 = 247;
pub const GREY_MID: u8 = 243;
pub const GREY_DIM: u8 = 239;

/// init-layout.ts:131: `styleText("cyan", s)`.
pub fn accent(s: &str, on: bool) -> String {
    todo!()
}

/// init-layout.ts:135-137.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpKey {
    pub key: String,
    pub does: String,
}

/// init-layout.ts:141: plain at depth 1 or under, `styleText("dim")` under 8 (painted only where `on`), grey 243 from 8.
pub fn muted(s: &str, depth: u8, on: bool) -> String {
    todo!()
}

/// init-layout.ts:144-148.
pub fn help_line(keys: &[HelpKey], depth: u8, on: bool) -> String {
    todo!()
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

/// What a screen reads off the process it runs in: whether styleText paints, the colour depth of the output, the
/// glyph set, and the process stdout's columns that clack wraps a frame at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    pub on: bool,
    pub unicode: bool,
    pub stdout_columns: Option<u16>,
}

/// init-layout.ts:156-159 over clack's `log.message` (@clack/prompts index.mjs:726-748): one write of the bar line,
/// the bold title on the green submit glyph, and each line down the gray bar, a line wider than
/// `width_of(columns, 100) - 3` once its codes are out wrapped by `wrap`.
pub fn card(title: &str, lines: &[String], output: &dyn ScreenOut, paint: &Paint) {
    todo!()
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
    pub input: tokio::sync::mpsc::UnboundedReceiver<String>,
    pub output: Arc<dyn ScreenOut>,
    pub paint: Paint,
    /// Ends the prompt from outside, as a cancel.
    pub signal: Option<futures::future::BoxFuture<'static, ()>>,
}

/// init-layout.ts:196-218: marker on No unless `initial` is true; left/right move it, `y` and `n` answer at once,
/// enter takes the marked answer, a lone escape and ctrl-c cancel.
pub async fn confirm_prompt(o: PromptOptions, initial: Option<bool>) -> Answer<bool> {
    todo!()
}

/// init-layout.ts:221-232: every character drawn as the mask, the text never written; enter on nothing is `""`.
pub async fn password_prompt(o: PromptOptions) -> Answer<String> {
    todo!()
}

/// Whether the stream is a terminal (init-layout.ts:116); `None` is no stream.
pub fn is_tty(output: Option<&dyn ScreenOut>) -> bool {
    todo!()
}
