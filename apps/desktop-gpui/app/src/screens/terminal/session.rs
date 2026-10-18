// SPDX-License-Identifier: AGPL-3.0-only
//! One shell: the pty it reads on its computer's daemon, and the screen libghostty-vt keeps of it. The daemon's
//! channel feeds it output; the view draws its engine and hands it the person's keys and the size it fits.

use gpui_kit::{Context, EventEmitter, SharedString};

use super::engine::Engine;

/// Bytes or a size the shell owes its pty, taken by the channel that holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Write(String),
    Resize { cols: u16, rows: u16 },
}

/// What the view and the pane redraw on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changed {
    /// New output reached the screen.
    Output,
    /// The title, an exit or a loss.
    State,
}

/// The pty's termios mode as the daemon last reported it (`pty.mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Canonical: the shell reads whole lines, so typing is composed here and sent on Enter.
    Line,
    Raw,
}

/// `compose.ts`: the local line a line-mode pty has not been sent yet, painted on the screen while it echoes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Compose {
    mode: Mode,
    echo: bool,
    buffer: String,
}

/// Until the daemon reports a pty's mode it is treated as raw.
const RAW: Compose = Compose { mode: Mode::Raw, echo: true, buffer: String::new() };

/// What one key or mode report does: bytes for the pty and bytes painted on the local screen.
#[derive(Debug, Default, PartialEq, Eq)]
struct Step {
    to_pty: String,
    to_screen: String,
}

fn printable(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| !(c < ' ' || c == '\x7f' || ('\u{80}'..='\u{9f}').contains(&c)))
}

/// One cell per code point: a wide glyph leaves a stray cell until the pty repaints the line.
fn erase(text: &str) -> String {
    "\x08 \x08".repeat(text.chars().count())
}

impl Compose {
    fn painted(&self) -> &str {
        if self.mode == Mode::Line && self.echo {
            &self.buffer
        } else {
            ""
        }
    }

    fn key(&mut self, key: &str) -> Step {
        if self.mode == Mode::Raw {
            return Step { to_pty: key.to_owned(), to_screen: String::new() };
        }
        let shown = erase(self.painted());
        if key == "\r" {
            let line = std::mem::take(&mut self.buffer);
            return Step { to_pty: line + "\r", to_screen: shown };
        }
        if (key == "\x7f" || key == "\x08") && !self.buffer.is_empty() {
            self.buffer.pop();
            return Step { to_pty: String::new(), to_screen: if self.echo { "\x08 \x08".into() } else { String::new() } };
        }
        if printable(key) {
            self.buffer.push_str(key);
            return Step { to_pty: String::new(), to_screen: if self.echo { key.to_owned() } else { String::new() } };
        }
        let line = std::mem::take(&mut self.buffer);
        Step { to_pty: line + key, to_screen: shown }
    }

    /// A mode report: leaving line mode sends any pending line first.
    fn mode(&mut self, mode: Mode, echo: bool) -> Step {
        let before = self.painted().to_owned();
        let to_pty = if mode == Mode::Raw { std::mem::take(&mut self.buffer) } else { String::new() };
        self.mode = mode;
        self.echo = echo;
        let after = self.painted();
        Step { to_pty, to_screen: if before == after { String::new() } else { erase(&before) + after } }
    }
}

pub struct Session {
    pty: SharedString,
    title: SharedString,
    engine: Engine,
    exited: Option<i32>,
    /// Why the shell is gone with its pty still listed, such as the daemon restarting.
    lost: Option<SharedString>,
    compose: Compose,
    /// Whether the daemon has reported the mode since the last attach.
    reported: bool,
    /// Between a wipe and the attach's answer the daemon replays old output, whose queries were answered long ago.
    replaying: bool,
    /// A line typed and not sent when the channel dropped; it comes back only if the re-attach reports line mode.
    held: String,
    /// The size the pty was last told, which it was created or listed at to begin with.
    told: Option<(u16, u16)>,
    outbox: Vec<Out>,
}

impl EventEmitter<Changed> for Session {}

impl Session {
    pub fn new(pty: impl Into<SharedString>, title: impl Into<SharedString>, cols: u16, rows: u16) -> Session {
        Session {
            pty: pty.into(),
            title: title.into(),
            engine: Engine::new(cols, rows),
            exited: None,
            lost: None,
            compose: RAW,
            reported: false,
            replaying: false,
            held: String::new(),
            told: Some((cols, rows)),
            outbox: Vec::new(),
        }
    }

    pub fn pty(&self) -> &SharedString {
        &self.pty
    }

    pub fn title(&self) -> &SharedString {
        &self.title
    }

    pub fn exited(&self) -> Option<i32> {
        self.exited
    }

    pub fn lost(&self) -> Option<&SharedString> {
        self.lost.as_ref()
    }

    /// The pty's mode as last reported, None before the daemon said.
    pub fn mode(&self) -> Option<Mode> {
        self.reported.then_some(self.compose.mode)
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Output from the pty: onto the screen, and any reply the screen owes back to the pty.
    pub fn feed(&mut self, data: &[u8], cx: &mut Context<Self>) {
        self.engine.write(data);
        let replies = self.engine.take_replies();
        if !replies.is_empty() && !self.replaying {
            self.outbox.push(Out::Write(String::from_utf8_lossy(&replies).into_owned()));
        }
        cx.emit(Changed::Output);
        cx.notify();
    }

    /// Typed or pasted bytes, already encoded for the program, on their way to the pty; a line-mode pty composes them
    /// here first, as today's pane does (`compose.ts`).
    pub fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if self.exited.is_some() || self.lost.is_some() || bytes.is_empty() {
            return;
        }
        let step = self.compose.key(&String::from_utf8_lossy(bytes));
        self.apply(step, cx);
    }

    fn apply(&mut self, step: Step, cx: &mut Context<Self>) {
        if !step.to_screen.is_empty() {
            self.engine.write(step.to_screen.as_bytes());
            cx.emit(Changed::Output);
        }
        if !step.to_pty.is_empty() {
            self.outbox.push(Out::Write(step.to_pty));
        }
        cx.notify();
    }

    /// The daemon's `pty.mode` report; a line held over a reconnect comes back once it reports line mode again.
    pub fn set_mode(&mut self, mode: Mode, echo: bool, cx: &mut Context<Self>) {
        let line = std::mem::take(&mut self.held);
        self.reported = true;
        let step = self.compose.mode(mode, echo);
        self.apply(step, cx);
        if !line.is_empty() && self.compose.mode == Mode::Line {
            self.compose.buffer = line.clone();
            let step = Step { to_pty: String::new(), to_screen: if self.compose.echo { line } else { String::new() } };
            self.apply(step, cx);
        }
    }

    /// The grid the view fits, which the screen reflows to at once; the pty hears it from [`Self::tell_size`].
    pub fn resize(&mut self, cols: u16, rows: u16, cell_w: u32, cell_h: u32, cx: &mut Context<Self>) {
        if self.engine.size() == (cols, rows) {
            return;
        }
        self.engine.resize(cols, rows, cell_w, cell_h);
        cx.notify();
    }

    /// Tells the pty the screen's size once more, as after a re-attach, whatever it was told before.
    pub fn tell_size_again(&mut self, cx: &mut Context<Self>) {
        self.told = None;
        self.tell_size(cx);
    }

    /// Passes the screen's size on to the pty, once the view's grid has settled.
    pub fn tell_size(&mut self, cx: &mut Context<Self>) {
        let (cols, rows) = self.engine.size();
        if self.told == Some((cols, rows)) {
            return;
        }
        self.told = Some((cols, rows));
        self.outbox.push(Out::Resize { cols, rows });
        cx.notify();
    }

    /// Wipes the screen ahead of a reconnect's replay of the daemon's scrollback; the mode is unknown until the
    /// re-attach reports it.
    pub fn reset_screen(&mut self, cx: &mut Context<Self>) {
        if self.compose.mode == Mode::Line {
            self.held = std::mem::take(&mut self.compose.buffer);
        }
        self.compose = RAW;
        self.reported = false;
        self.replaying = true;
        self.engine.reset();
        cx.emit(Changed::Output);
        cx.notify();
    }

    /// An attach is going out: the daemon replays the pty's kept output ahead of its answer.
    pub fn begin_replay(&mut self) {
        self.replaying = true;
    }

    /// The attach answered: the replay is over, and what the screen answers from now on goes to the pty.
    pub fn end_replay(&mut self) {
        self.replaying = false;
    }

    /// What the channel owes the pty since it last asked.
    pub fn take_out(&mut self) -> Vec<Out> {
        std::mem::take(&mut self.outbox)
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        cx.emit(Changed::State);
        cx.notify();
    }

    /// The process ended, as `link.ts` marks it: a note in its scrollback, and no more typing.
    pub fn set_exited(&mut self, code: i32, cx: &mut Context<Self>) {
        if self.exited.is_some() {
            return;
        }
        self.exited = Some(code);
        self.engine.write(b"\r\n[process exited]\r\n");
        cx.emit(Changed::State);
        cx.notify();
    }

    /// The shell is gone for `why`, which also lands in its scrollback as `[<why>]`, as today's pane prints it.
    pub fn set_lost(&mut self, why: impl Into<SharedString>, cx: &mut Context<Self>) {
        let why = why.into();
        self.engine.write(format!("\r\n[{why}]\r\n").as_bytes());
        self.exited.get_or_insert(-1);
        self.lost = Some(why);
        cx.emit(Changed::State);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(echo: bool) -> Compose {
        Compose { mode: Mode::Line, echo, buffer: String::new() }
    }

    #[test]
    fn a_raw_pty_takes_every_key_as_it_comes() {
        let mut c = RAW;
        assert_eq!(c.key("a"), Step { to_pty: "a".into(), to_screen: String::new() });
        assert_eq!(c.key("\x03"), Step { to_pty: "\x03".into(), to_screen: String::new() });
    }

    #[test]
    fn a_line_mode_pty_gets_the_whole_line_on_enter_and_the_screen_echoes_meanwhile() {
        let mut c = line(true);
        assert_eq!(c.key("l"), Step { to_pty: String::new(), to_screen: "l".into() });
        assert_eq!(c.key("x"), Step { to_pty: String::new(), to_screen: "x".into() });
        assert_eq!(c.key("\x7f"), Step { to_pty: String::new(), to_screen: "\x08 \x08".into() });
        assert_eq!(c.key("s"), Step { to_pty: String::new(), to_screen: "s".into() });
        assert_eq!(c.key("\r"), Step { to_pty: "ls\r".into(), to_screen: "\x08 \x08\x08 \x08".into() });
        assert_eq!(c.buffer, "");
    }

    #[test]
    fn a_control_key_sends_the_pending_line_ahead_of_itself() {
        let mut c = line(false);
        c.key("ec");
        assert_eq!(c.key("\t"), Step { to_pty: "ec\t".into(), to_screen: String::new() });
    }

    #[test]
    fn leaving_line_mode_sends_what_was_typed() {
        let mut c = line(true);
        c.key("vi");
        assert_eq!(c.mode(Mode::Raw, true), Step { to_pty: "vi".into(), to_screen: "\x08 \x08\x08 \x08".into() });
    }
}
