// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's shell block run where it stands (`replyRun.ts`) and the box under the block while it runs
//! (`InlineRun.tsx`): the command runs in a pty of the workspace's own daemon over the terminal's channel, and the
//! thread records each step through the host (`sessions.run`). Kept outside the view: a block scrolled out of the
//! list is not drawn, and its run still ends and still has to be recorded.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    div, prelude::*, px, App, AsyncApp, Context, Entity, Focusable as _, FontFeatures, Global, SharedString, StyleRefinement, Subscription,
    Task, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionsRunRequest;

use super::markdown::{OnRun, RunAction};
use crate::screens::terminal::hub::{self, Key};
use crate::screens::terminal::link::run::{terminal_text, ReplyEvent, ReplyPty, RunExit};
use crate::screens::terminal::link::{Link, LinkHold, Status, Target};
use crate::screens::terminal::look::Look;
use crate::screens::terminal::view::TerminalView;
use crate::screens::Host;
use crate::thread_view::ThreadView;
use crate::transcript::{fence_openings, Row, Run};
use crate::ui::ActivePalette;

const RUN_BLOCK: &str = include_str!("../../../../../../packages/protocol/src/run-block.ts");

/// `RUN_BOX_PX`: the height of a run's box under its block, live or finished; it scrolls.
pub const RUN_BOX: gpui_kit::Pixels = px(232.);
/// `RUN_MOVE_AFTER_MS`: how long a run prints before it is offered a terminal tab.
pub const MOVE_AFTER: Duration = Duration::from_millis(3000);

/// `PTY_RUN_DAEMON_VERSION`: the first daemon that runs a command in a pty, read from its source.
pub fn pty_run_daemon_version() -> u64 {
    let at = RUN_BLOCK.find("PTY_RUN_DAEMON_VERSION =").map(|at| &RUN_BLOCK[at..]).unwrap_or_default();
    at.split(['=', ';']).nth(1).and_then(|n| n.trim().parse().ok()).unwrap_or(u64::MAX)
}

/// `RUN_WORDS` and the refusals of replyRun.ts.
pub mod words {
    pub const RUN: &str = "Run";
    pub const RUN_AGAIN: &str = "Run again";
    pub const RUNNING: &str = "Running";
    pub const MOVE: &str = "Move to terminal";
    pub const SEND: &str = "Send output to the agent";
    pub const OPEN_TAB: &str = "Open the tab";
    /// `RUN_NO_LINK_LINE`.
    pub const NO_LINK: &str = "this workspace's terminal is not connected yet, so nothing can run in it; try again once it is";

    /// `runDaemonBehindLine`.
    pub fn behind(version: Option<u64>) -> String {
        let version = version.map_or_else(|| "unknown".to_owned(), |v| v.to_string());
        format!(
            "wsp on this workspace is version {version}, older than {}, the first that runs a reply's command; update the workspace and run it again",
            super::pty_run_daemon_version()
        )
    }
}

/// The runs this app watches to their end, by run id, and those a press in it started, whose terminal takes the
/// keyboard when it first shows; one moved to a tab stays, marked, so nothing takes its pty back as a run.
#[derive(Default)]
struct Watching {
    runs: HashMap<String, Watched>,
    focusing: HashSet<String>,
}

struct Watched {
    pty: Option<Entity<ReplyPty>>,
    moved: bool,
    _hold: Option<LinkHold>,
}

impl Watched {
    fn new(pty: Entity<ReplyPty>, hold: LinkHold) -> Watched {
        Watched { pty: Some(pty), moved: false, _hold: Some(hold) }
    }
}

impl Global for Watching {}

/// The pty a run this app watches is running in.
pub fn held(run_id: &str, cx: &App) -> Option<Entity<ReplyPty>> {
    cx.try_global::<Watching>()?.runs.get(run_id).filter(|w| !w.moved).and_then(|w| w.pty.clone())
}

/// Whether this run's terminal takes the keyboard as it shows: true once, for a run a press here started.
fn take_focus(run_id: &str, cx: &mut App) -> bool {
    cx.default_global::<Watching>().focusing.remove(run_id)
}

/// The reply a block sits in (`ReplyRunScope`): where its run starts and how the thread records it.
#[derive(Clone)]
pub struct Scope {
    host: Host,
    workspace: String,
    thread: String,
    turn: String,
    message: String,
    cwd: Option<String>,
    view: WeakEntity<ThreadView>,
}

impl Scope {
    /// The scope of `row`, an agent's reply in the thread `view` follows; none while it follows none.
    pub fn of(view: &ThreadView, row: &Row, cx: &Context<ThreadView>) -> Option<Scope> {
        Some(Scope {
            host: view.host()?.clone(),
            workspace: view.followed_workspace()?.to_owned(),
            thread: view.followed()?.to_owned(),
            turn: row.turn.clone().unwrap_or_default(),
            message: row.key.clone(),
            cwd: view.transcript().cwd().map(str::to_owned),
            view: cx.entity().downgrade(),
        })
    }

    /// `runBlockKey`: the reply's message and where its `block`th fenced block opens in its text, in UTF-16 units.
    fn block_key(&self, block: usize, cx: &App) -> Option<String> {
        let view = self.view.upgrade()?;
        let view = view.read(cx);
        let row = view.rows().iter().find(|row| row.key == self.message)?;
        let at = fence_openings(&row.text).get(block)?.at;
        Some(format!("{}@{at}", self.message))
    }

    fn drawer(&self) -> Key {
        Key::Thread { workspace_id: self.workspace.clone(), thread_id: self.thread.clone() }
    }
}

/// What a press under a reply's blocks does, for the blocks of a part of the reply that `before` fenced blocks precede.
pub fn on_run(scope: Option<Scope>, before: usize) -> OnRun {
    Arc::new(move |action, _, cx| {
        let Some(scope) = scope.as_ref() else { return };
        let whole = |run: Run| Run { block: run.block + before, ..run };
        match action {
            RunAction::Start { block, command, said } => start(scope, before + block, command, said, cx),
            RunAction::Send { message } => _ = scope.view.update(cx, |view, cx| view.send(message, cx)),
            RunAction::Move(run) => move_run(scope, whole(run), cx),
            RunAction::Open(run) => open_tab(scope, &run, cx),
            RunAction::Resume(run) => resume(scope, whole(run), cx),
        }
    })
}

/// One step of a run as the thread records it (`RunStep`).
#[derive(Clone)]
struct Step {
    host: Host,
    thread: String,
    turn: String,
    run_id: String,
    block: String,
    command: String,
}

impl Step {
    fn of(scope: &Scope, run_id: &str, block: &str, command: &str) -> Step {
        Step {
            host: scope.host.clone(),
            thread: scope.thread.clone(),
            turn: scope.turn.clone(),
            run_id: run_id.to_owned(),
            block: block.to_owned(),
            command: command.to_owned(),
        }
    }

    /// `recordRun`: `sessions.run` with this step's `state` and what it carries.
    async fn record(&self, state: &str, more: Value) -> Result<(), String> {
        let mut step = json!({
            "threadId": self.thread, "turnId": self.turn, "runId": self.run_id, "block": self.block, "command": self.command,
            "state": state,
        });
        if let (Some(step), Value::Object(more)) = (step.as_object_mut(), more) {
            step.extend(more);
        }
        let request: SessionsRunRequest = serde_json::from_value(step).map_err(|e| e.to_string())?;
        self.host.run(move |client| async move { client.call(request).await.map(|_| ()) }).await.map_err(|f| f.message)
    }
}

/// Waits while the channel first dials, and with `reconnect` while it dials again; true once it is live, false
/// where it is refused, unanswered, or reconnecting and not waited for.
async fn live(link: &Entity<Link>, reconnect: bool, cx: &mut AsyncApp) -> bool {
    loop {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let (status, _changed) = cx.update(|cx| {
            let mut tx = Some(tx);
            let changed = cx.subscribe(link, move |_, _, _| _ = tx.take().map(|tx| tx.send(())));
            (link.read(cx).status().clone(), changed)
        });
        match status {
            Status::Live => return true,
            Status::Connecting { .. } if !reconnect => return false,
            Status::Opening | Status::Connecting { .. } => {
                if rx.await.is_err() {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// `startRun`: runs the block's command in a pty of its own and records it running; the end is recorded when it
/// comes. Refused before anything starts where the workspace's channel or daemon cannot run it, said under the block.
fn start(scope: &Scope, block: usize, command: String, said: Entity<Option<SharedString>>, cx: &mut App) {
    let Some(block) = scope.block_key(block, cx) else { return };
    said.update(cx, |said, cx| {
        *said = None;
        cx.notify();
    });
    let link = Link::get(&scope.host, Target::Workspace(scope.workspace.clone()), cx);
    let hold = link.update(cx, |link, _| link.hold());
    let scope = scope.clone();
    cx.spawn(async move |cx| {
        if let Err(why) = begin(&scope, &link, hold, &block, &command, cx).await {
            said.update(cx, |said, cx| {
                *said = Some(why.into());
                cx.notify();
            });
        }
    })
    .detach();
}

async fn begin(scope: &Scope, link: &Entity<Link>, hold: LinkHold, block: &str, command: &str, cx: &mut AsyncApp) -> Result<(), String> {
    if !live(link, false, cx).await {
        return Err(words::NO_LINK.to_owned());
    }
    let version = link.read_with(cx, |link, _| link.daemon_version());
    if version.is_none_or(|v| v < pty_run_daemon_version()) {
        return Err(words::behind(version));
    }
    let run_id = crate::screens::composer::link::request_id();
    let (cwd, thread) = (scope.cwd.clone(), scope.thread.clone());
    let started = link.update(cx, |link, cx| link.run(command, cwd.as_deref(), Some(&thread), cx));
    let pty = started.await.map_err(|why| why.to_string())?;
    let pty_id = pty.read_with(cx, |pty, cx| pty.session().read(cx).pty().to_string());
    let step = Step::of(scope, &run_id, block, command);
    // Held before the record goes: its event can reach the block, which then draws, before the answer does.
    cx.update(|cx| {
        let watching = cx.default_global::<Watching>();
        watching.runs.insert(run_id.clone(), Watched::new(pty.clone(), hold));
        watching.focusing.insert(run_id.clone());
    });
    if let Err(why) = step.record("running", json!({ "ptyId": pty_id })).await {
        // A run no thread records is one no block can draw or end: the command goes with the refusal.
        cx.update(|cx| {
            let watching = cx.default_global::<Watching>();
            watching.runs.remove(&run_id);
            watching.focusing.remove(&run_id);
            link.update(cx, |link, cx| link.forget_run(&pty_id, cx));
        });
        return Err(why);
    }
    watch(step, link.clone(), pty, cx);
    Ok(())
}

/// `resumeRun`: takes back a run the thread records running, after a relaunch or in another window, and watches it
/// to its end; a run whose pty the daemon no longer holds is recorded lost. One this window watches is left alone.
fn resume(scope: &Scope, run: Run, cx: &mut App) {
    let Some(pty_id) = run.pty_id.clone().filter(|_| run.state == "running") else { return };
    if cx.try_global::<Watching>().is_some_and(|w| w.runs.contains_key(&run.run_id)) {
        return;
    }
    let Some(block) = scope.block_key(run.block, cx) else { return };
    let link = Link::get(&scope.host, Target::Workspace(scope.workspace.clone()), cx);
    let hold = link.update(cx, |link, _| link.hold());
    let step = Step::of(scope, &run.run_id, &block, &run.command);
    let thread = scope.thread.clone();
    cx.spawn(async move |cx| {
        if !live(&link, true, cx).await {
            return;
        }
        let resumed = link.update(cx, |link, cx| link.resume_run(&pty_id, Some(&thread), cx));
        if !resumed.await {
            _ = step.record("lost", json!({})).await;
            return;
        }
        let Some(pty) = link.read_with(cx, |link, cx| link.reply_pty(&pty_id, cx)) else { return };
        let fresh = cx.update(|cx| {
            let watching = cx.default_global::<Watching>();
            if watching.runs.contains_key(&step.run_id) {
                return false;
            }
            watching.runs.insert(step.run_id.clone(), Watched::new(pty.clone(), hold));
            true
        });
        if fresh {
            watch(step, link, pty, cx);
        }
    })
    .detach();
}

/// Records the run's end once it comes, with the text it printed, and lets its pty go; one moved to a tab ends there.
fn watch(step: Step, link: Entity<Link>, pty: Entity<ReplyPty>, cx: &mut AsyncApp) {
    let ended = pty.update(cx, |pty, _| pty.ended());
    cx.spawn(async move |cx| {
        let Ok(exit) = ended.await else { return };
        let moved = cx.update(|cx| {
            let watching = cx.default_global::<Watching>();
            let moved = watching.runs.get(&step.run_id).is_none_or(|w| w.moved);
            if !moved {
                watching.runs.remove(&step.run_id);
            }
            moved
        });
        if moved {
            return;
        }
        let RunExit { code, signal, lost } = exit;
        let (output, pty_id) = pty.read_with(cx, |pty, cx| (terminal_text(pty.mirror()), pty.session().read(cx).pty().to_string()));
        if lost {
            _ = step.record("lost", json!({})).await;
            link.update(cx, |link, cx| link.forget_run(&pty_id, cx));
            return;
        }
        let mut ended = json!({ "exitCode": code, "output": output });
        if let Some(signal) = signal {
            ended["signal"] = signal.into();
        }
        _ = step.record("exited", ended).await;
        link.update(cx, |link, cx| link.forget_run(&pty_id, cx));
    })
    .detach();
}

/// `moveRun`: hands a running run's pty to the thread's terminal tabs, the process untouched, records it moved and
/// opens that tab.
fn move_run(scope: &Scope, run: Run, cx: &mut App) {
    let Some(pty_id) = run.pty_id.clone() else { return };
    let Some(link) = Link::find(&Target::Workspace(scope.workspace.clone()), cx) else { return };
    let Some(block) = scope.block_key(run.block, cx) else { return };
    if let Some(watched) = cx.default_global::<Watching>().runs.get_mut(&run.run_id) {
        watched.moved = true;
    }
    let step = Step::of(scope, &run.run_id, &block, &run.command);
    let key = scope.drawer();
    cx.spawn(async move |cx| {
        let moved = link.update(cx, |link, cx| link.move_to_tab(&pty_id, cx));
        if moved.await.is_err() {
            return;
        }
        cx.update(|cx| {
            let ui = hub::drawer(&key, cx).new_terminal(&pty_id).set_open(true);
            hub::set_drawer(&key, ui, cx);
            if let Some(watched) = cx.default_global::<Watching>().runs.get_mut(&step.run_id) {
                watched.pty = None;
                watched._hold = None;
            }
            cx.refresh_windows();
        });
        _ = step.record("moved", json!({ "ptyId": pty_id })).await;
    })
    .detach();
}

/// Open the tab: the thread's drawer, opened on the tab a moved run went to.
fn open_tab(scope: &Scope, run: &Run, cx: &mut App) {
    let Some(pty_id) = run.pty_id.as_deref() else { return };
    let key = scope.drawer();
    let ui = hub::drawer(&key, cx).set_open(true);
    let ui = if ui.terminal_ids.iter().any(|t| t == pty_id) { ui.activate(pty_id) } else { ui.new_terminal(pty_id) };
    hub::set_drawer(&key, ui, cx);
    cx.refresh_windows();
}

/// `LiveRun`: the run's terminal in a 232 px box taking the person's keys, Running under it, and Move to terminal
/// once it has printed for three seconds. A program that takes the whole screen moves itself to a tab.
pub struct LiveRun {
    run: Run,
    on_run: OnRun,
    view: Option<(Entity<ReplyPty>, Entity<TerminalView>)>,
    movable: bool,
    resumed: bool,
    _alt: Option<Subscription>,
    _watching: Subscription,
    _movable: Task<()>,
}

impl LiveRun {
    pub fn new(run: Run, on_run: OnRun, cx: &mut Context<Self>) -> LiveRun {
        let movable = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MOVE_AFTER).await;
            _ = this.update(cx, |this, cx| {
                this.movable = true;
                cx.notify();
            });
        });
        LiveRun {
            run,
            on_run,
            view: None,
            movable: false,
            resumed: false,
            _alt: None,
            _watching: cx.observe_global::<Watching>(|_, cx| cx.notify()),
            _movable: movable,
        }
    }

    /// The run the box draws, as the thread now records it; a press's handler, too.
    pub fn set(&mut self, run: &Run, on_run: OnRun, cx: &mut Context<Self>) {
        if self.run.run_id != run.run_id {
            *self = LiveRun::new(run.clone(), on_run, cx);
            cx.notify();
            return;
        }
        self.on_run = on_run;
        if self.run != *run {
            self.run = run.clone();
            cx.notify();
        }
    }

    fn terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<TerminalView>> {
        let pty = held(&self.run.run_id, cx);
        match (&self.view, pty) {
            (Some((shown, view)), Some(pty)) if *shown == pty => Some(view.clone()),
            (_, None) => {
                self.view = None;
                if !std::mem::replace(&mut self.resumed, true) {
                    let (on_run, run) = (self.on_run.clone(), self.run.clone());
                    window.defer(cx, move |window, cx| on_run(RunAction::Resume(run), window, cx));
                }
                None
            }
            (_, Some(pty)) => {
                let session = pty.read(cx).session().clone();
                let view = cx.new(|cx| TerminalView::new(session, Look::new(None, None, cx), window, cx));
                if take_focus(&self.run.run_id, cx) {
                    view.read(cx).focus_handle(cx).focus(window, cx);
                }
                self._alt = Some(cx.subscribe_in(&pty, window, |this, _, event: &ReplyEvent, window, cx| {
                    if *event == ReplyEvent::AltScreen {
                        (this.on_run)(RunAction::Move(this.run.clone()), window, cx);
                    }
                }));
                if pty.read(cx).alt_screen() {
                    let (on_run, run) = (self.on_run.clone(), self.run.clone());
                    window.defer(cx, move |window, cx| on_run(RunAction::Move(run), window, cx));
                }
                self.view = Some((pty, view.clone()));
                Some(view)
            }
        }
    }
}

impl Render for LiveRun {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let terminal = self.terminal(window, cx);
        let (on_run, run) = (self.on_run.clone(), self.run.clone());
        v_flex()
            .w_full()
            .debug_selector(|| "run-live".into())
            .child(div().w_full().h(RUN_BOX).bg(p.terminal_background).children(terminal))
            .child(footer(words::RUNNING.into(), cx).child(h_flex().gap(px(4.)).when(self.movable, |el| {
                el.child(run_button("run-move", words::MOVE, move |_, window, cx| on_run(RunAction::Move(run.clone()), window, cx)))
            })))
    }
}

/// `RunFooter`: how the run stands at the left in 12 px muted tabular figures, its presses at the right.
pub fn footer(words: SharedString, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let mut tabular = StyleRefinement::default();
    tabular.text.font_features = Some(FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    h_flex()
        .w_full()
        .min_h(px(36.))
        .justify_between()
        .gap(px(12.))
        .py(px(4.))
        .pr(px(6.))
        .pl(px(12.))
        .font_family(gpui_kit::component::ActiveTheme::theme(cx).font_family.clone())
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .refine_style(&tabular)
        .child(div().debug_selector(|| "run-state".into()).min_w_0().child(words))
}

/// A run's ghost `xs` button, found by its id.
pub fn run_button(
    id: &'static str,
    words: &'static str,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div().debug_selector(move || id.into()).child(crate::ui::Button::ghost(id, words).xs().on_click(on_click))
}

#[cfg(test)]
mod tests {
    use super::{pty_run_daemon_version, words, MOVE_AFTER};

    fn source(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn the_runs_words_and_numbers_are_todays() {
        let block = source("packages/protocol/src/run-block.ts");
        for word in [words::RUN, words::RUN_AGAIN, words::RUNNING, words::MOVE, words::SEND, words::OPEN_TAB] {
            assert!(block.contains(&format!("\"{word}\"")), "{word} is not in RUN_WORDS");
        }
        assert!(block.contains(&format!("RUN_MOVE_AFTER_MS = {};", MOVE_AFTER.as_millis())));
        assert_eq!(pty_run_daemon_version(), 98);
        let run = source("apps/web/src/components/chat/replyRun.ts");
        assert!(run.contains(&format!("\"{}\"", words::NO_LINK)), "RUN_NO_LINK_LINE");
        let behind = words::behind(Some(97));
        assert!(behind.starts_with("wsp on this workspace is version 97, older than 98, "), "{behind}");
        assert!(run.contains("the first that runs a reply's command; update the workspace and run it again`"), "runDaemonBehindLine");
        assert!(words::behind(None).contains("version unknown,"));
    }
}
