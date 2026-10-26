// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's shell block run where it stands, against a host started for each test alone carrying the wsp-daemon
//! scripts/stage-daemon.sh staged: Run under the block runs the command in a pty over the terminal's channel, the
//! thread records each step (`sessions.run`), and the block draws what the record says. Skips, saying why, where no
//! daemon is staged.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{ask, wsp_daemon, Throwaway};
use desktop_gpui::screens::terminal::hub::{self, Key};
use desktop_gpui::screens::terminal::link::{Link, Status, Tag, Target};
use desktop_gpui::screens::thread_view::markdown::RunAction;
use desktop_gpui::screens::thread_view::run;
use desktop_gpui::screens::Host;
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::{RowKind, Run};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest};
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(60);

/// A claude whose reply to each prompt is a shell block: `quick` prints and ends, `long` keeps going, `full` takes the
/// whole screen and keeps going.
const ANSWERS: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r line; do
  case "$line" in
    *quick*) text='Run:\n\n```sh\necho wsp-$((6*7))\n```' ;;
    *long*) text='Run:\n\n```sh\necho started; sleep 60\n```' ;;
    *full*) text='Run:\n\n```sh\nprintf \"\\033[?1049h\"; sleep 60\n```' ;;
    *) text='ok' ;;
  esac
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"%s"}]}}\n' "$session" "$text"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"%s"}\n' "$text" "$session"
done
"#;

struct Started {
    server: Throwaway,
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    item: ThreadItem,
}

impl Started {
    fn on(prompt: &str) -> Option<Started> {
        if let Err(why) = wsp_daemon() {
            println!("skipped: {why}; run apps/desktop-gpui/scripts/stage-daemon.sh");
            return None;
        }
        let server = Throwaway::start_with(ANSWERS, true);
        let folder = server.repo("repo");
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", prompt, "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = Arc::new(runtime.block_on(server.client()));
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let view = threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).unwrap_or_else(|| panic!("no thread for {prompt}"));
        let item = ThreadItem::of(view).expect("a lead thread");
        Some(Started { server, client, runtime, item })
    }

    fn host(&self) -> Host {
        Host::new(self.client.clone(), self.runtime.clone())
    }

    /// The ptys the workspace's daemon lists, read on a channel of the test's own.
    fn listed(&self) -> Vec<Value> {
        let (client, workspace) = (self.client.clone(), self.item.workspace_id.clone());
        self.runtime.block_on(async move {
            let opened = client
                .call(ask::<DaemonOpenRequest>(json!({ "workspaceId": workspace })))
                .await
                .unwrap_or_else(|f| panic!("{}", f.message));
            let frame = json!({ "channel": opened.channel, "frame": { "op": "pty.list" } });
            let reply = client.call(ask::<DaemonSendRequest>(frame)).await.unwrap_or_else(|f| panic!("{}", f.message)).reply;
            client.call(ask::<DaemonCloseRequest>(json!({ "channel": opened.channel }))).await.unwrap_or_else(|f| panic!("{}", f.message));
            reply["ptys"].as_array().cloned().unwrap_or_default()
        })
    }
}

/// The thread's view in a window, following the thread, once its reply's block draws Run.
fn opened<'a>(started: &Started, cx: &'a mut TestAppContext) -> (Entity<ThreadView>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    let host = started.host();
    view.update(cx, |v, cx| v.follow(&host, &started.item, false, cx));
    until(cx, started, "the reply's block with Run", |cx| cx.debug_bounds("code-run").is_some());
    (view, cx)
}

/// Lets real time pass, the host's pushes landing on the window's thread, until `done` holds.
fn until(cx: &mut VisualTestContext, started: &Started, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s\nhost log: {}", WAIT.as_secs(), started.server.log());
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The run the thread records for its reply's block, as the view reads it.
fn recorded(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> Option<Run> {
    view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|row| match &row.kind {
            RowKind::Agent { runs, .. } => runs.first().cloned(),
            _ => None,
        })
    })
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("no {selector}")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn link(started: &Started, cx: &mut VisualTestContext) -> Entity<Link> {
    let target = Target::Workspace(started.item.workspace_id.clone());
    cx.update(|_, cx| Link::find(&target, cx)).expect("the workspace's channel")
}

#[gpui_kit::test]
fn run_runs_the_block_where_it_stands_and_the_thread_records_how_it_ended(cx: &mut TestAppContext) {
    let Some(started) = Started::on("quick") else { return };
    let (view, cx) = opened(&started, cx);
    click(cx, "code-run");
    until(cx, &started, "the run recorded exited", |cx| recorded(&view, cx).is_some_and(|r| r.state == "exited"));
    let run = recorded(&view, cx).unwrap();
    assert_eq!((run.command.as_str(), run.output.as_deref(), run.exit_code), ("echo wsp-$((6*7))", Some("wsp-42"), Some(0)));
    assert!(cx.debug_bounds("code-run-output").is_some() && cx.debug_bounds("run-send").is_some(), "the ended run under its block");
    let first = run.run_id.clone();
    until(cx, &started, "the finished run's pty let go", |_| started.listed().iter().all(|p| p["reply"] != true));

    click(cx, "code-run");
    until(cx, &started, "Run again recorded as a second run", |cx| {
        recorded(&view, cx).is_some_and(|r| r.run_id != first && r.state == "exited")
    });
}

#[gpui_kit::test]
fn a_long_run_draws_its_terminal_and_moves_to_a_terminal_tab_still_running(cx: &mut TestAppContext) {
    let Some(started) = Started::on("long") else { return };
    let (view, cx) = opened(&started, cx);
    click(cx, "code-run");
    until(cx, &started, "the run recorded running", |cx| recorded(&view, cx).is_some_and(|r| r.state == "running"));
    let run = recorded(&view, cx).unwrap();
    let pty = run.pty_id.clone().expect("the running record names its pty");
    until(cx, &started, "the live box", |cx| cx.debug_bounds("run-live").is_some());
    assert!(cx.debug_bounds("run-live").unwrap().size.height >= px(232.), "the live terminal is 232 px tall");
    until(cx, &started, "started printed in the live terminal", |cx| {
        cx.update(|_, cx| run::held(&run.run_id, cx).is_some_and(|p| p.read(cx).mirror().contains("started")))
    });
    assert!(started.listed().iter().any(|p| p["id"] == pty.as_str() && p["reply"] == true), "the daemon holds it as a reply's pty");
    assert!(cx.debug_bounds("run-move").is_none(), "Move to terminal before three seconds");
    cx.executor().advance_clock(run::MOVE_AFTER);
    until(cx, &started, "Move to terminal", |cx| cx.debug_bounds("run-move").is_some());
    let reply_pty = cx.update(|_, cx| run::held(&run.run_id, cx)).expect("the run's pty").downgrade();
    click(cx, "run-move");
    until(cx, &started, "the run recorded moved", |cx| recorded(&view, cx).is_some_and(|r| r.state == "moved"));
    until(cx, &started, "the moved run's own pty let go here, its tab keeping the session", |_| reply_pty.upgrade().is_none());
    // A block drawn again from a record still running, as one drawn late after the move, takes nothing back.
    let scope = view.update(cx, |v, cx| {
        let row = v.rows().iter().find(|r| matches!(r.kind, RowKind::Agent { .. })).cloned().expect("the reply");
        run::Scope::of(v, &row, cx)
    });
    let on_run = run::on_run(scope, 0);
    cx.update(|window, cx| on_run(RunAction::Resume(run.clone()), window, cx));
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
    let link = link(&started, cx);
    assert!(link.read_with(cx, |l, cx| l.reply_pty(&pty, cx).is_none()), "a moved run's pty was taken back as a run");
    let tag = link.read_with(cx, |l, cx| l.ptys().iter().find(|p| p.session.read(cx).pty() == pty.as_str()).map(|p| p.tag.clone()));
    assert_eq!(tag, Some(Tag::Thread(started.item.thread_id.clone())), "the pty is the thread's next shell");
    let key = Key::Thread { workspace_id: started.item.workspace_id.clone(), thread_id: started.item.thread_id.clone() };
    let ui = cx.update(|_, cx| hub::drawer(&key, cx));
    assert!(ui.terminal_open && ui.active_terminal_id == pty, "the thread's drawer opened on the moved run: {ui:?}");
    assert!(started.listed().iter().any(|p| p["id"] == pty.as_str() && p["reply"] != true), "the daemon still runs it, as a tab");
    assert!(cx.debug_bounds("run-open").is_some(), "Open the tab under the block");
    link.update(cx, |l, cx| l.kill(&pty, cx));
}

#[gpui_kit::test]
fn a_run_that_takes_the_whole_screen_moves_itself_to_a_terminal_tab(cx: &mut TestAppContext) {
    let Some(started) = Started::on("full") else { return };
    let (view, cx) = opened(&started, cx);
    click(cx, "code-run");
    until(cx, &started, "the run recorded moved with no press", |cx| recorded(&view, cx).is_some_and(|r| r.state == "moved"));
    let pty = recorded(&view, cx).and_then(|r| r.pty_id).expect("the moved record names its pty");
    assert!(started.listed().iter().any(|p| p["id"] == pty.as_str() && p["exited"] != true), "still running in its tab");
    let link = link(&started, cx);
    link.update(cx, |l, cx| l.kill(&pty, cx));
}

#[gpui_kit::test]
fn run_while_the_channel_reconnects_is_refused_at_once(cx: &mut TestAppContext) {
    let Some(started) = Started::on("quick") else { return };
    let (view, cx) = opened(&started, cx);
    let (host, target) = (started.host(), Target::Workspace(started.item.workspace_id.clone()));
    let link = cx.update(|_, cx| Link::get(&host, target, cx));
    let _held = link.update(cx, |l, _| l.hold());
    until(cx, &started, "the channel live", |cx| link.read_with(cx, |l, _| *l.status() == Status::Live));
    link.update(cx, |l, cx| l.drop_for_test(cx));
    click(cx, "code-run");
    until(cx, &started, "the refusal under the block", |cx| cx.debug_bounds("run-refusal").is_some());
    assert!(recorded(&view, cx).is_none(), "a run was recorded while the channel was down");
}
