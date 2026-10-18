// SPDX-License-Identifier: AGPL-3.0-only
//! One shell's screen, kept by libghostty-vt (the engine today's pane runs as wasm): bytes from the pty go in, and a
//! [`Frame`] of rows, cells, colours and the cursor comes out for the renderer to draw, with the selection, links and
//! the key, mouse and paste encoders the view drives.

use std::cell::RefCell;
use std::rc::Rc;

use libghostty_vt::error::Error;
use libghostty_vt::key;
use libghostty_vt::mouse;
use libghostty_vt::render::{CellIterator, CursorVisualStyle, Dirty, RenderState, RowIterator};
use libghostty_vt::screen::{CellWide, Screen};
use libghostty_vt::selection::{FormatOptions, SelectLineOptions, SelectWordOptions, Selection};
use libghostty_vt::style::{PaletteIndex, RgbColor, Underline};
use libghostty_vt::terminal::{CursorStyle, Mode, Point, PointCoordinate, PointSpace, ScrollViewport, Terminal};

/// MAX_SCROLLBACK_ROWS in `terminal/ghostty/core.ts`, which Ghostty reads as bytes, about one page of rows.
pub const MAX_SCROLLBACK: usize = 10_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<RgbColor> for Rgb {
    fn from(c: RgbColor) -> Self {
        Rgb(c.r, c.g, c.b)
    }
}

impl From<Rgb> for RgbColor {
    fn from(c: Rgb) -> Self {
        RgbColor { r: c.0, g: c.1, b: c.2 }
    }
}

/// How a cell is drawn besides its colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: Line,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Line {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// A cell of a row: its text is `Row::text[start..start + len]`, empty for a blank or a wide character's tail. Its
/// colours are resolved as today's snapshot resolves them: defaults filled in, inverse swapped, faint blended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub start: u32,
    pub len: u32,
    /// 1, 2 for a wide character, 0 for the tail a wide character covers.
    pub width: u8,
    pub fg: Rgb,
    pub bg: Rgb,
    pub flags: Flags,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub cells: Vec<Cell>,
    /// The row continues the one above it after a soft wrap.
    pub wrap_continuation: bool,
    /// The row soft-wraps onto the one below it.
    pub wraps: bool,
}

impl Row {
    pub fn cell_text(&self, cell: &Cell) -> &str {
        &self.text[cell.start as usize..cell.start as usize + cell.len as usize]
    }

    /// The row's text as it reads, a wide character's tail adding nothing and a blank cell a space.
    pub fn plain(&self) -> String {
        self.cells.iter().filter(|c| c.width > 0).map(|c| if c.len == 0 { " " } else { self.cell_text(c) }).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
    Hollow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
    pub blinking: bool,
    /// The cursor colour in force: the program's (OSC 12), else the default the look set, else the foreground.
    pub color: Rgb,
    pub wide: bool,
}

/// What the renderer draws: the visible rows, the colours in force and where the cursor is.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub cols: u16,
    pub rows: Vec<Row>,
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Option<Cursor>,
    pub scrollbar: Scroll,
}

/// The viewport within the whole buffer, in rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

impl Scroll {
    pub fn at_bottom(&self) -> bool {
        self.offset + self.len >= self.total
    }

    /// Scrollback exists once the buffer holds more rows than the viewport shows.
    pub fn has_scrollback(&self) -> bool {
        self.total > self.len
    }
}

/// The colours a config or theme gives the screen before a program changes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Option<Rgb>,
    /// Palette entries by index; the rest keep Ghostty's defaults.
    pub palette: Vec<(u8, Rgb)>,
}

/// A point in the whole buffer, which output scrolling the viewport does not move.
pub type ScreenPoint = PointCoordinate;

/// A key as the encoder takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInput {
    pub action: key::Action,
    pub key: key::Key,
    pub mods: key::Mods,
    pub consumed: key::Mods,
    pub utf8: Option<String>,
    pub unshifted: char,
}

/// A mouse event as the encoder takes it, in pixels from the terminal's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseInput {
    pub action: mouse::Action,
    pub button: Option<mouse::Button>,
    pub mods: key::Mods,
    pub x: f32,
    pub y: f32,
    pub size: mouse::EncoderSize,
    pub any_pressed: bool,
}

pub struct Engine {
    term: Terminal<'static, 'static>,
    state: RenderState<'static>,
    row_iter: RowIterator<'static>,
    cell_iter: CellIterator<'static>,
    /// Replies the screen owes the pty (device status, mode reports), collected while bytes are written.
    replies: Rc<RefCell<Vec<u8>>>,
    keys: key::Encoder<'static>,
    key_event: key::Event<'static>,
    mice: mouse::Encoder<'static>,
    mouse_event: mouse::Event<'static>,
    cursor_style: Option<CursorStyle>,
    cursor_blink: bool,
}

fn grow<T>(mut f: impl FnMut(&mut Vec<u8>) -> Result<T, Error>, size: usize) -> Option<(T, Vec<u8>)> {
    let mut buf = vec![0; size];
    loop {
        match f(&mut buf) {
            Ok(v) => return Some((v, buf)),
            Err(Error::OutOfSpace { required }) if required > buf.len() => buf.resize(required, 0),
            Err(_) => return None,
        }
    }
}

impl Engine {
    pub fn new(cols: u16, rows: u16) -> Engine {
        let mut term = Terminal::new(cols.max(1), rows.max(1)).expect("libghostty-vt makes a terminal");
        _ = term.set_scrollback_max_bytes(Some(MAX_SCROLLBACK));
        let replies = Rc::new(RefCell::new(Vec::new()));
        let sink = replies.clone();
        _ = term.on_pty_write(move |_, data: &[u8]| sink.borrow_mut().extend_from_slice(data));
        let made = "libghostty-vt makes its encoders";
        let mut engine = Engine {
            term,
            state: RenderState::new().expect("libghostty-vt makes a render state"),
            row_iter: RowIterator::new().expect("libghostty-vt makes a row iterator"),
            cell_iter: CellIterator::new().expect("libghostty-vt makes a cell iterator"),
            replies,
            keys: key::Encoder::new().expect(made),
            key_event: key::Event::new().expect(made),
            mice: mouse::Encoder::new().expect(made),
            mouse_event: mouse::Event::new().expect(made),
            cursor_style: None,
            cursor_blink: true,
        };
        engine.apply_cursor_defaults();
        engine
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
    }

    /// The bytes the screen answered with since the last call, for the pty.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.replies.borrow_mut())
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.cols().unwrap_or(1), self.term.rows().unwrap_or(1))
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell_w: u32, cell_h: u32) {
        if self.size() != (cols, rows) {
            _ = self.term.resize(cols.max(1), rows.max(1), cell_w.max(1), cell_h.max(1));
        }
    }

    pub fn reset(&mut self) {
        self.term.reset();
        self.apply_cursor_defaults();
    }

    /// Lays the look's colours under whatever a program set: OSC 4 changes to the palette survive.
    pub fn set_colors(&mut self, colors: &Colors) {
        _ = self.term.set_default_fg_color(Some(colors.foreground.into()));
        _ = self.term.set_default_bg_color(Some(colors.background.into()));
        _ = self.term.set_default_cursor_color(colors.cursor.map(RgbColor::from));
        if let Ok(mut palette) = self.term.color_palette() {
            for &(i, c) in &colors.palette {
                palette.set(PaletteIndex(i), c.into());
            }
            _ = self.term.set_default_color_palette(Some(palette));
        }
    }

    /// The config's cursor style and blink, which a reset (RIS) drops and this lays again.
    pub fn set_cursor_defaults(&mut self, style: Option<CursorStyle>, blink: bool) {
        self.cursor_style = style;
        self.cursor_blink = blink;
        self.apply_cursor_defaults();
    }

    fn apply_cursor_defaults(&mut self) {
        _ = self.term.set_default_cursor_blink(Some(self.cursor_blink));
        if self.cursor_style.is_some() {
            _ = self.term.set_default_cursor_style(self.cursor_style);
        }
    }

    pub fn scroll(&mut self, to: ScrollViewport) {
        self.term.scroll_viewport(to);
    }

    pub fn scrollbar(&self) -> Scroll {
        self.term.scrollbar().map(|b| Scroll { total: b.total, offset: b.offset, len: b.len }).unwrap_or_default()
    }

    pub fn scrollback_rows(&self) -> usize {
        self.term.scrollback_rows().unwrap_or(0)
    }

    pub fn alternate_screen(&self) -> bool {
        matches!(self.term.active_screen(), Ok(Screen::Alternate))
    }

    pub fn mouse_tracking(&self) -> bool {
        self.term.is_mouse_tracking().unwrap_or(false)
    }

    pub fn mode(&self, mode: Mode) -> bool {
        self.term.mode(mode).unwrap_or(false)
    }

    /// The program asked for key releases (Kitty's report-events flag).
    pub fn reports_releases(&self) -> bool {
        self.term.kitty_keyboard_flags().is_ok_and(|f| f.contains(key::KittyKeyFlags::REPORT_EVENTS))
    }

    pub fn title(&self) -> Option<String> {
        self.term.title().ok().filter(|t| !t.is_empty()).map(str::to_owned)
    }

    /// The shell's folder as OSC 7 last said it.
    pub fn pwd(&self) -> Option<String> {
        self.term.pwd().ok().filter(|p| !p.is_empty()).map(str::to_owned)
    }

    pub fn terminal(&self) -> &Terminal<'static, 'static> {
        &self.term
    }

    /// The buffer point under a viewport cell.
    pub fn screen_point(&self, x: u16, y: u16) -> Option<ScreenPoint> {
        let at = self.term.grid_ref(Point::Viewport(PointCoordinate { x, y: y as u32 })).ok()?;
        self.term.point_from_grid_ref(&at, PointSpace::Screen).ok().flatten()
    }

    /// Selects from `a` to `b`, both ends inclusive, in buffer points.
    pub fn select(&mut self, a: ScreenPoint, b: ScreenPoint) {
        let (Ok(a), Ok(b)) = (self.term.grid_ref(Point::Screen(a)), self.term.grid_ref(Point::Screen(b))) else { return };
        _ = self.term.set_selection(Some(&Selection::new(a, b, false)));
    }

    pub fn clear_selection(&mut self) {
        _ = self.term.set_selection(None);
    }

    pub fn has_selection(&self) -> bool {
        self.term.selection().ok().flatten().is_some()
    }

    fn range(&self, sel: Option<Selection<'_>>) -> Option<(ScreenPoint, ScreenPoint)> {
        let sel = sel?;
        let a = self.term.point_from_grid_ref(&sel.start(), PointSpace::Screen).ok().flatten()?;
        let b = self.term.point_from_grid_ref(&sel.end(), PointSpace::Screen).ok().flatten()?;
        Some((a, b))
    }

    /// The word under a viewport cell, as Ghostty bounds it.
    pub fn word_at(&self, x: u16, y: u16) -> Option<(ScreenPoint, ScreenPoint)> {
        let at = self.term.grid_ref(Point::Viewport(PointCoordinate { x, y: y as u32 })).ok()?;
        self.range(self.term.select_word(SelectWordOptions::new(at)).ok().flatten())
    }

    /// The soft-wrapped line under a viewport cell.
    pub fn line_at(&self, x: u16, y: u16) -> Option<(ScreenPoint, ScreenPoint)> {
        let at = self.term.grid_ref(Point::Viewport(PointCoordinate { x, y: y as u32 })).ok()?;
        self.range(self.term.select_line(SelectLineOptions::new(at)).ok().flatten())
    }

    /// The selection as plain text, soft wraps joined and trailing blanks trimmed, as today's copy reads it.
    pub fn selection_text(&self) -> Option<String> {
        let options = || FormatOptions::new().with_emit_format(libghostty_vt::fmt::Format::Plain).with_unwrap(true).with_trim(true);
        let (written, buf) = grow(|buf| self.term.format_selection_buf(options(), buf), 4096)?;
        let written = written?;
        Some(String::from_utf8_lossy(&buf[..written]).into_owned())
    }

    /// The OSC 8 address of a viewport cell.
    pub fn hyperlink_at(&self, x: u16, y: u16) -> Option<String> {
        let at = self.term.grid_ref(Point::Viewport(PointCoordinate { x, y: y as u32 })).ok()?;
        let (len, buf) = grow(|buf| at.hyperlink_uri(buf), 256)?;
        (len > 0).then(|| String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    /// The bytes a key sends the program, by the encoder's rules for the modes the program set.
    pub fn encode_key(&mut self, input: &KeyInput) -> Vec<u8> {
        // Today's wasm never ran Ghostty's macOS branches, so Option acted as Alt there: Option+b is ESC b.
        self.keys.set_options_from_terminal(&self.term).set_macos_option_as_alt(key::OptionAsAlt::True);
        self.key_event
            .set_action(input.action)
            .set_key(input.key)
            .set_mods(input.mods)
            .set_consumed_mods(input.consumed)
            .set_composing(false)
            .set_utf8(input.utf8.clone())
            .set_unshifted_codepoint(input.unshifted);
        let mut out = Vec::new();
        _ = self.keys.encode_to_vec(&self.key_event, &mut out);
        out
    }

    /// The bytes a mouse event reports to a program that tracks the mouse.
    pub fn encode_mouse(&mut self, input: &MouseInput) -> Vec<u8> {
        self.mice
            .set_options_from_terminal(&self.term)
            .set_size(input.size)
            .set_any_button_pressed(input.any_pressed)
            .set_track_last_cell(true);
        self.mouse_event
            .set_action(input.action)
            .set_button(input.button)
            .set_mods(input.mods)
            .set_position(mouse::Position { x: input.x, y: input.y });
        let mut out = Vec::new();
        _ = self.mice.encode_to_vec(&self.mouse_event, &mut out);
        out
    }

    /// Pasted text as the program takes it: unsafe controls blanked, bracketed when the program asked.
    pub fn encode_paste(&self, text: &str) -> Vec<u8> {
        let mut data = text.as_bytes().to_vec();
        let bracketed = self.mode(Mode::BRACKETED_PASTE);
        grow(|buf| libghostty_vt::paste::encode(&mut data, bracketed, buf), text.len() + 16)
            .map(|(n, mut buf)| {
                buf.truncate(n);
                buf
            })
            .unwrap_or_default()
    }

    /// Reads the screen as it stands into `frame`, reusing its rows' buffers and reading again only the rows that
    /// changed since the last call.
    pub fn frame(&mut self, frame: &mut Frame) {
        let Ok(snap) = self.state.update(&self.term) else { return };
        let cols = snap.cols().unwrap_or(0);
        let rows = snap.rows().unwrap_or(0) as usize;
        let mut full = frame.cols != cols || frame.rows.len() != rows;
        if let Ok(c) = snap.colors() {
            let (fg, bg) = (Rgb::from(c.foreground), Rgb::from(c.background));
            full |= frame.foreground != fg || frame.background != bg;
            frame.foreground = fg;
            frame.background = bg;
        }
        frame.cols = cols;
        frame.rows.resize_with(rows, Row::default);
        let dirty = snap.dirty().unwrap_or(Dirty::Full);
        full |= dirty == Dirty::Full;
        frame.cursor = snap.cursor().ok().and_then(|c| {
            let at = c.viewport.filter(|_| c.visible)?;
            Some(Cursor {
                x: at.x,
                y: at.y,
                shape: match c.visual_style {
                    CursorVisualStyle::Bar => CursorShape::Bar,
                    CursorVisualStyle::Underline => CursorShape::Underline,
                    CursorVisualStyle::BlockHollow => CursorShape::Hollow,
                    _ => CursorShape::Block,
                },
                blinking: c.blinking,
                color: snap.cursor_color().ok().flatten().map(Rgb::from).unwrap_or(frame.foreground),
                wide: at.at_wide_tail,
            })
        });
        if full || dirty != Dirty::Clean {
            let (fg0, bg0) = (frame.foreground, frame.background);
            let mut scratch = String::new();
            if let Ok(mut iter) = self.row_iter.update(&snap) {
                let mut y = 0;
                while let Some(row) = iter.next() {
                    if y >= rows {
                        break;
                    }
                    if full || row.dirty().unwrap_or(true) {
                        let out = &mut frame.rows[y];
                        out.text.clear();
                        out.cells.clear();
                        let raw = row.raw_row().ok();
                        out.wrap_continuation = raw.and_then(|r| r.is_wrap_continuation().ok()).unwrap_or(false);
                        out.wraps = raw.and_then(|r| r.is_wrapped().ok()).unwrap_or(false);
                        let selected = row.selection().ok().flatten();
                        if let Ok(mut cells) = self.cell_iter.update(row) {
                            let mut x: u16 = 0;
                            while let Some(cell) = cells.next() {
                                let start = out.text.len();
                                scratch.clear();
                                if cell.graphemes_len().unwrap_or(0) > 0 && cell.graphemes_utf8(&mut scratch).is_ok() {
                                    out.text.push_str(&scratch);
                                }
                                let width = match cell.raw_cell().and_then(|c| c.wide()) {
                                    Ok(CellWide::Wide) => 2,
                                    Ok(CellWide::SpacerTail) => 0,
                                    _ => 1,
                                };
                                let mut flags = Flags::default();
                                if cell.has_styling().unwrap_or(false) {
                                    if let Ok(s) = cell.style() {
                                        flags = Flags {
                                            bold: s.bold,
                                            italic: s.italic,
                                            faint: s.faint,
                                            inverse: s.inverse,
                                            invisible: s.invisible,
                                            strikethrough: s.strikethrough,
                                            overline: s.overline,
                                            underline: match s.underline {
                                                Underline::Single => Line::Single,
                                                Underline::Double => Line::Double,
                                                Underline::Curly => Line::Curly,
                                                Underline::Dotted => Line::Dotted,
                                                Underline::Dashed => Line::Dashed,
                                                _ => Line::None,
                                            },
                                            selected: false,
                                        };
                                    }
                                }
                                flags.selected = selected.is_some_and(|s| x >= s.start_x && x <= s.end_x);
                                let mut fg = cell.fg_color().ok().flatten().map(Rgb::from).unwrap_or(fg0);
                                let mut bg = cell.bg_color().ok().flatten().map(Rgb::from).unwrap_or(bg0);
                                if flags.inverse {
                                    std::mem::swap(&mut fg, &mut bg);
                                }
                                if flags.faint {
                                    let mix = |f: u8, b: u8| ((f as u32 * 155 + b as u32 * 100) / 255) as u8;
                                    fg = Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2));
                                }
                                out.cells.push(Cell { start: start as u32, len: (out.text.len() - start) as u32, width, fg, bg, flags });
                                x += 1;
                            }
                        }
                        _ = row.set_dirty(false);
                    }
                    y += 1;
                }
            }
            _ = snap.clean();
        }
        frame.scrollbar = self.scrollbar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(engine: &mut Engine) -> Vec<String> {
        let mut frame = Frame::default();
        engine.frame(&mut frame);
        frame.rows.iter().map(|r| r.plain().trim_end().to_owned()).collect()
    }

    fn colors() -> Colors {
        Colors {
            foreground: Rgb(10, 20, 30),
            background: Rgb(250, 250, 250),
            cursor: Some(Rgb(1, 2, 3)),
            palette: vec![(1, Rgb(200, 0, 0))],
        }
    }

    #[test]
    fn text_lands_in_rows_with_its_colours() {
        let mut e = Engine::new(20, 4);
        e.set_colors(&colors());
        e.write(b"\x1b[31mred\x1b[0m plain\r\nnext");
        let mut frame = Frame::default();
        e.frame(&mut frame);
        assert_eq!(frame.rows.len(), 4);
        assert_eq!(frame.rows[0].plain().trim_end(), "red plain");
        assert_eq!(frame.rows[1].plain().trim_end(), "next");
        assert_eq!(frame.rows[0].cells[0].fg, Rgb(200, 0, 0), "SGR 31 reads the palette's slot 1");
        assert_eq!(frame.rows[0].cells[4].fg, Rgb(10, 20, 30), "after SGR 0 the default foreground");
        assert_eq!(frame.rows[0].cells[4].bg, Rgb(250, 250, 250));
        let cursor = frame.cursor.expect("the cursor shows");
        assert_eq!((cursor.x, cursor.y, cursor.color), (4, 1, Rgb(1, 2, 3)));
    }

    #[test]
    fn inverse_swaps_and_faint_blends_toward_the_background() {
        let mut e = Engine::new(20, 2);
        e.set_colors(&colors());
        e.write(b"\x1b[7mi\x1b[0m\x1b[2mf");
        let mut frame = Frame::default();
        e.frame(&mut frame);
        let (i, f) = (frame.rows[0].cells[0], frame.rows[0].cells[1]);
        assert_eq!((i.fg, i.bg), (Rgb(250, 250, 250), Rgb(10, 20, 30)));
        assert_eq!(f.fg, Rgb(104, 110, 116));
    }

    #[test]
    fn a_wide_character_takes_two_cells_and_a_cluster_joins_only_when_the_program_asks() {
        let mut e = Engine::new(12, 2);
        e.write("a世b👍🏽".as_bytes());
        let mut frame = Frame::default();
        e.frame(&mut frame);
        let widths: Vec<u8> = frame.rows[0].cells.iter().take(8).map(|c| c.width).collect();
        assert_eq!(widths, [1, 2, 0, 1, 2, 0, 2, 0], "today's pane leaves mode 2027 off, so the modifier takes its own cells");
        assert_eq!(frame.rows[0].plain().trim_end(), "a世b👍🏽");
        e.write("\r\n\x1b[?2027h👍🏽".as_bytes());
        e.frame(&mut frame);
        assert_eq!(frame.rows[1].cell_text(&frame.rows[1].cells[0]).chars().count(), 2, "with 2027 the tone joins its base");
        assert_eq!(frame.rows[1].cells[2].len, 0);
    }

    #[test]
    fn a_device_status_query_is_answered_for_the_pty() {
        let mut e = Engine::new(10, 2);
        e.write(b"\x1b[6n");
        assert_eq!(e.take_replies(), b"\x1b[1;1R");
        assert!(e.take_replies().is_empty());
    }

    #[test]
    fn a_reset_keeps_the_looks_palette() {
        let mut e = Engine::new(10, 2);
        e.set_colors(&colors());
        e.write(b"\x1b]4;1;rgb:00/00/ff\x1b\\");
        e.reset();
        e.write(b"\x1b[31mx");
        let mut frame = Frame::default();
        e.frame(&mut frame);
        assert_eq!(frame.rows[0].cells[0].fg, Rgb(200, 0, 0), "RIS drops the program's slot 1 and keeps the default the look set");
    }

    #[test]
    fn only_changed_rows_are_read_again() {
        let mut e = Engine::new(10, 3);
        e.write(b"one\r\ntwo");
        let mut frame = Frame::default();
        e.frame(&mut frame);
        frame.rows[0].text.push('!');
        e.write(b"\r\nthree");
        e.frame(&mut frame);
        assert_eq!(frame.rows[0].text, "one!", "a clean row keeps its buffer");
        assert_eq!(frame.rows[2].plain().trim_end(), "three");
    }

    #[test]
    fn output_past_the_screen_goes_to_scrollback_and_the_view_follows() {
        let mut e = Engine::new(10, 3);
        for i in 0..10 {
            e.write(format!("l{i}\r\n").as_bytes());
        }
        assert_eq!(screen(&mut e), ["l8", "l9", ""]);
        assert!(e.scrollback_rows() >= 8);
        assert!(e.scrollbar().has_scrollback());
        e.scroll(ScrollViewport::Top);
        assert_eq!(screen(&mut e)[0], "l0");
        assert!(!e.scrollbar().at_bottom());
    }

    #[test]
    fn a_selection_by_points_words_and_lines_copies_its_text() {
        let mut e = Engine::new(20, 3);
        e.write(b"hello brave world\r\nnext line");
        let (a, b) = (e.screen_point(6, 0).unwrap(), e.screen_point(10, 0).unwrap());
        e.select(a, b);
        assert_eq!(e.selection_text().as_deref(), Some("brave"));
        let (a, b) = e.word_at(13, 0).unwrap();
        e.select(a, b);
        assert_eq!(e.selection_text().as_deref(), Some("world"));
        let (a, b) = e.line_at(2, 1).unwrap();
        e.select(a, b);
        assert_eq!(e.selection_text().as_deref(), Some("next line"));
        let mut frame = Frame::default();
        e.frame(&mut frame);
        assert!(frame.rows[1].cells[0].flags.selected && !frame.rows[0].cells[0].flags.selected);
        e.clear_selection();
        assert_eq!(e.selection_text(), None);
    }

    #[test]
    fn an_osc_8_link_is_read_off_its_cells() {
        let mut e = Engine::new(30, 2);
        e.write(b"\x1b]8;;https://wsp.dev/x\x1b\\here\x1b]8;;\x1b\\ plain");
        assert_eq!(e.hyperlink_at(1, 0).as_deref(), Some("https://wsp.dev/x"));
        assert_eq!(e.hyperlink_at(6, 0), None);
    }

    #[test]
    fn keys_follow_the_programs_modes() {
        let mut e = Engine::new(10, 2);
        let up = KeyInput {
            action: key::Action::Press,
            key: key::Key::ArrowUp,
            mods: key::Mods::empty(),
            consumed: key::Mods::empty(),
            utf8: None,
            unshifted: '\0',
        };
        assert_eq!(e.encode_key(&up), b"\x1b[A");
        e.write(b"\x1b[?1h");
        assert_eq!(e.encode_key(&up), b"\x1bOA");
        let ctrl_c = KeyInput { key: key::Key::C, mods: key::Mods::CTRL, utf8: Some("c".into()), unshifted: 'c', ..up };
        assert_eq!(e.encode_key(&ctrl_c), b"\x03");
    }

    #[test]
    fn a_paste_is_bracketed_only_when_the_program_asked() {
        let mut e = Engine::new(10, 2);
        assert_eq!(e.encode_paste("a\nb"), b"a\rb");
        e.write(b"\x1b[?2004h");
        assert_eq!(e.encode_paste("a\nb"), b"\x1b[200~a\nb\x1b[201~");
    }
}
