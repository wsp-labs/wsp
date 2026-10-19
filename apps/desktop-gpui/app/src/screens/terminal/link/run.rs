// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's block run on the channel (`run`, `resumeRun`, `runExit`, `moveToTab` and `forgetRun` in today's
//! `terminal/link.ts`): a pty of its own the channel routes and lists as no tab until it is moved to one.

use futures::channel::oneshot;
use gpui_kit::{App, AppContext, AsyncApp, Context, Entity, EventEmitter, SharedString, Task, WeakEntity};
use serde_json::{json, Value};

use super::{answered, Link, Tag};
use crate::screens::terminal::session::{Changed, Session};

/// `RUN_ROWS`: the rows a run opens at, before the box under its block has measured it.
pub const RUN_ROWS: u16 = 12;
/// `MIRROR_CAP`: the most of a run's output kept to read once it ends, as the daemon caps a pty's scrollback.
const MIRROR_CAP: usize = 256 * 1024;

/// How a run's process ended (`RunExit`): `lost` where the daemon no longer held its pty and nobody saw the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunExit {
    pub code: i64,
    pub signal: Option<i64>,
    pub lost: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyEvent {
    /// The program took the whole screen.
    AltScreen,
    Exited,
}

/// A run's pty as this window holds it: its screen, what it printed, and how it ended.
pub struct ReplyPty {
    session: Entity<Session>,
    thread: Option<String>,
    mirror: String,
    exit: Option<RunExit>,
    waiting: Vec<oneshot::Sender<RunExit>>,
    alt: bool,
}

impl EventEmitter<ReplyEvent> for ReplyPty {}

impl ReplyPty {
    pub fn session(&self) -> &Entity<Session> {
        &self.session
    }

    pub fn exit(&self) -> Option<RunExit> {
        self.exit
    }

    pub fn alt_screen(&self) -> bool {
        self.alt
    }

    /// Everything the pty printed that is still kept, as the terminal received it (`mirrorText`).
    pub fn mirror(&self) -> &str {
        &self.mirror
    }

    /// Answers once the process ends, at once for one already over (`runExit`).
    pub fn ended(&mut self) -> oneshot::Receiver<RunExit> {
        let (tx, rx) = oneshot::channel();
        match self.exit {
            Some(exit) => _ = tx.send(exit),
            None => self.waiting.push(tx),
        }
        rx
    }

    fn exited(&mut self, exit: RunExit, cx: &mut Context<Self>) {
        if self.exit.is_some() {
            return;
        }
        self.exit = Some(exit);
        for tx in self.waiting.drain(..) {
            _ = tx.send(exit);
        }
        cx.emit(ReplyEvent::Exited);
        cx.notify();
    }

    fn saw(&mut self, data: &[u8], alt: bool, cx: &mut Context<Self>) {
        self.mirror.push_str(&String::from_utf8_lossy(data));
        if self.mirror.len() > MIRROR_CAP {
            let mut cut = self.mirror.len() - MIRROR_CAP;
            while !self.mirror.is_char_boundary(cut) {
                cut += 1;
            }
            self.mirror.drain(..cut);
        }
        if alt && !self.alt {
            self.alt = true;
            cx.emit(ReplyEvent::AltScreen);
        }
    }

    fn replay(&mut self, cx: &mut Context<Self>) {
        self.mirror.clear();
        self.session.update(cx, |s, cx| s.reset_screen(cx));
    }
}

/// The runs a channel holds, and the daemon's version from its hello.
#[derive(Default)]
pub(super) struct Runs {
    held: Vec<Entity<ReplyPty>>,
    version: Option<u64>,
}

impl Runs {
    pub(super) fn sessions<'a>(&'a self, cx: &'a App) -> impl Iterator<Item = &'a Entity<Session>> + 'a {
        self.held.iter().map(move |run| run.read(cx).session())
    }

    fn find(&self, pty: &str, cx: &App) -> Option<Entity<ReplyPty>> {
        self.held.iter().find(|run| run.read(cx).session.read(cx).pty() == pty).cloned()
    }

    pub(super) fn holds(&self, pty: &str, cx: &App) -> bool {
        self.find(pty, cx).is_some()
    }

    /// After `data` reached a run's screen: kept, and the screen's switch to the alternate one heard.
    pub(super) fn fed(&self, pty: &str, data: &[u8], cx: &mut App) {
        let Some(run) = self.find(pty, cx) else { return };
        let alt = run.read(cx).session.read(cx).engine().alternate_screen();
        run.update(cx, |run, cx| run.saw(data, alt, cx));
    }

    /// A `pty.exit`: true where it was a run's, whose output is kept as text with no line of ours joined to it.
    pub(super) fn exited(&self, event: &Value, cx: &mut App) -> bool {
        let Some(run) = event["ptyId"].as_str().and_then(|pty| self.find(pty, cx)) else { return false };
        let exit = RunExit { code: event["exitCode"].as_i64().unwrap_or(-1), signal: event["signal"].as_i64(), lost: false };
        run.update(cx, |run, cx| run.exited(exit, cx));
        true
    }

    /// The daemon's hello, which names its version: true where `event` is one.
    pub(super) fn hello(&mut self, event: &Value) -> bool {
        let hello = event["type"] == "daemon.hello";
        if hello {
            self.version = event["version"].as_u64();
        }
        hello
    }
}

impl Link {
    /// The protocol version the daemon's hello named, or none before one came or where it named none.
    pub fn daemon_version(&self) -> Option<u64> {
        self.runs.version
    }

    /// The run in `pty` this channel holds.
    pub fn reply_pty(&self, pty: &str, cx: &App) -> Option<Entity<ReplyPty>> {
        self.runs.find(pty, cx)
    }

    fn hold_run(&mut self, pty: &str, cols: u16, thread: Option<String>, cx: &mut Context<Self>) -> Entity<ReplyPty> {
        let session = cx.new(|_| Session::new(pty.to_owned(), "run", cols, RUN_ROWS));
        let drained = cx.subscribe(&session, |link, session, changed: &Changed, cx| {
            if *changed == Changed::Owed {
                link.drain(&session, cx);
            }
        });
        self.watched.insert(pty.to_owned().into(), drained);
        let run = cx.new(|_| ReplyPty { session, thread, mirror: String::new(), exit: None, waiting: Vec::new(), alt: false });
        self.runs.held.push(run.clone());
        run
    }

    fn let_go(&mut self, run: &Entity<ReplyPty>, cx: &mut Context<Self>) {
        self.runs.held.retain(|held| held != run);
        let pty = run.read(cx).session.read(cx).pty().clone();
        self.watched.remove(&pty);
    }

    /// Runs `command` in a pty of its own, held for its block and never a tab: the daemon runs it through the person's
    /// shell and the pty exits with it. Answers once the daemon took it and it is attached.
    pub fn run(
        &mut self,
        command: &str,
        cwd: Option<&str>,
        thread: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<ReplyPty>, SharedString>> {
        let (cols, _) = Link::last_size(cx);
        let mut params = json!({ "cols": cols, "rows": RUN_ROWS, "run": command });
        if let Some(cwd) = cwd {
            params["cwd"] = cwd.into();
        }
        if let Some(thread) = thread {
            params["thread"] = thread.into();
        }
        let Some(created) = self.ask("pty.create", params) else {
            return Task::ready(Err("not answering".into()));
        };
        let thread = thread.map(str::to_owned);
        cx.spawn(async move |this, cx| {
            let created = answered(created).await?;
            let pty = created["ptyId"].as_str().ok_or_else(|| SharedString::from("the daemon answered no ptyId"))?.to_owned();
            let closed = |_| SharedString::from("the terminal closed");
            let (run, attached) = this
                .update(cx, |link, cx| {
                    let run = link.hold_run(&pty, cols, thread, cx);
                    (run, link.ask("pty.attach", json!({ "ptyId": pty })))
                })
                .map_err(closed)?;
            let attached = match attached {
                Some(answer) => answered(answer).await,
                None => Err("not answering".into()),
            };
            if let Err(why) = attached {
                // The daemon took the command: unattached, it would run on as a reply's pty no listing shows.
                this.update(cx, |link, cx| {
                    link.let_go(&run, cx);
                    link.kill(&pty, cx);
                })
                .map_err(closed)?;
                return Err(why);
            }
            Ok(run)
        })
    }

    /// Takes back a run the thread's record names, after a relaunch or in another window: false where the daemon
    /// holds no such pty any more. One this channel holds already answers at once.
    pub fn resume_run(&mut self, pty: &str, thread: Option<&str>, cx: &mut Context<Self>) -> Task<bool> {
        if self.runs.holds(pty, cx) {
            return Task::ready(true);
        }
        if !self.proven {
            return Task::ready(false);
        }
        let (cols, _) = Link::last_size(cx);
        let run = self.hold_run(pty, cols, thread.map(str::to_owned), cx);
        let session = run.read(cx).session.clone();
        session.update(cx, |s, _| s.begin_replay());
        let Some(attached) = self.ask("pty.attach", json!({ "ptyId": pty })) else {
            self.let_go(&run, cx);
            return Task::ready(false);
        };
        cx.spawn(async move |this, cx| {
            let attached = answered(attached).await.is_ok();
            this.update(cx, |link, cx| {
                link.attach_answered(&session, attached, cx);
                if !attached {
                    link.let_go(&run, cx);
                }
                attached
            })
            .unwrap_or(false)
        })
    }

    /// Hands a running run's pty to the terminal tabs, the process untouched: the daemon stops marking it as a
    /// reply's, so every other window adopts it too, and here it is listed as the thread's next shell.
    pub fn move_to_tab(&mut self, pty: &str, cx: &mut Context<Self>) -> Task<Result<(), SharedString>> {
        let Some(run) = self.runs.find(pty, cx) else { return Task::ready(Err(format!("no pty {pty} here").into())) };
        let Some(tabbed) = self.ask("pty.tab", json!({ "ptyId": pty })) else {
            return Task::ready(Err("not answering".into()));
        };
        cx.spawn(async move |this, cx| {
            answered(tabbed).await?;
            this.update(cx, |link, cx| {
                link.let_go(&run, cx);
                // A run moved to a tab ends there: whoever waits on its end here hears it never comes.
                run.update(cx, |run, _| run.waiting.clear());
                let (session, thread) = {
                    let run = run.read(cx);
                    (run.session.clone(), run.thread.clone())
                };
                let title = link.numbered(cx);
                session.update(cx, |s, cx| s.set_title(title, cx));
                link.list(session, thread.map_or(Tag::None, Tag::Thread), cx);
            })
            .map_err(|_| SharedString::from("the terminal closed"))
        })
    }

    /// Lets go of a finished run: its text is on the thread now, so the pty goes.
    pub fn forget_run(&mut self, pty: &str, cx: &mut Context<Self>) {
        let Some(run) = self.runs.find(pty, cx) else { return };
        self.let_go(&run, cx);
        if let Some(answer) = self.ask("pty.kill", json!({ "ptyId": pty })) {
            cx.background_spawn(async move { _ = answer.await }).detach();
        }
    }

    /// After the channel came back: each run still going is attached again for the daemon to replay it, and one the
    /// daemon no longer holds ended unseen.
    pub(super) async fn reattach_runs(this: &WeakEntity<Link>, dial: u64, cx: &mut AsyncApp) {
        let Ok(held) = this.update(cx, |link, _| link.runs.held.clone()) else { return };
        for run in held {
            let Ok(attached) = this.update(cx, |link, cx| {
                if link.dial != dial || run.read(cx).exit.is_some() {
                    return None;
                }
                run.update(cx, |run, cx| run.replay(cx));
                let pty = run.read(cx).session.read(cx).pty().clone();
                link.ask("pty.attach", json!({ "ptyId": pty }))
            }) else {
                return;
            };
            let Some(attached) = attached else { continue };
            let attached = answered(attached).await.is_ok();
            let Ok(true) = this.update(cx, |link, cx| {
                if link.dial != dial {
                    return false;
                }
                let session = run.read(cx).session.clone();
                link.attach_answered(&session, attached, cx);
                if attached {
                    session.update(cx, |s, cx| s.tell_size_again(cx));
                } else {
                    run.update(cx, |run, cx| run.exited(RunExit { code: -1, signal: None, lost: true }, cx));
                }
                true
            }) else {
                return;
            };
        }
    }
}

/// `terminalText`: a terminal's bytes as the lines a person saw. Escape sequences carry no text, a carriage return
/// starts its line over, and a backspace takes the letter before it; trailing blank lines and spaces go.
pub fn terminal_text(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut line: Vec<char> = Vec::new();
    let mut col = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\x1b' => {
                let next = chars.get(i + 1).copied();
                if next == Some('[') {
                    let mut j = i + 2;
                    while j < chars.len() && !('\x40'..='\x7e').contains(&chars[j]) {
                        j += 1;
                    }
                    i = j;
                } else if matches!(next, Some(']' | 'P' | '_' | '^')) {
                    let mut j = i + 2;
                    while j < chars.len() && chars[j] != '\x07' && !(chars[j] == '\x1b' && chars.get(j + 1) == Some(&'\\')) {
                        j += 1;
                    }
                    i = if chars.get(j) == Some(&'\x07') { j } else { j + 1 };
                } else {
                    let mut j = i + 1;
                    while j < chars.len() && ('\x20'..='\x2f').contains(&chars[j]) {
                        j += 1;
                    }
                    i = j;
                }
            }
            '\r' => col = 0,
            '\n' => {
                out.push(line.drain(..).collect());
                col = 0;
            }
            '\x08' => {
                col = col.saturating_sub(1);
                line.truncate(col);
            }
            c if c < ' ' && c != '\t' => {}
            c => {
                if col < line.len() {
                    line[col] = c;
                } else {
                    line.resize(col, ' ');
                    line.push(c);
                }
                col += 1;
            }
        }
        i += 1;
    }
    out.push(line.into_iter().collect());
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::terminal_text;

    #[test]
    fn a_terminals_bytes_read_as_the_lines_a_person_saw() {
        assert_eq!(terminal_text("\x1b[1;32mok\x1b[0m\r\n"), "ok");
        assert_eq!(terminal_text("10%\r50%\r100%\n"), "100%");
        assert_eq!(terminal_text("ab\x08c\n"), "ac");
        assert_eq!(terminal_text("\x1b]0;title\x07hi\x1b]8;;u\x1b\\x\n\n  \n"), "hix");
        assert_eq!(terminal_text("\x1b(Bplain  \n"), "plain");
        assert_eq!(terminal_text("one\ntwo"), "one\ntwo");
    }
}
