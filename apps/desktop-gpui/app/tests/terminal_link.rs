// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal's daemon channel against a host started for each test alone, carrying the wsp-daemon
//! scripts/stage-daemon.sh staged: shells opened, typed into, resized, adopted, re-attached after a drop and ended
//! through `Link`, each checked against what the daemon itself lists. Skips, saying why, where no daemon is staged.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::FutureExt;

use common::{ask, wsp_daemon, Throwaway};
use desktop_gpui::screens::terminal::engine::Frame;
use desktop_gpui::screens::terminal::link::{Link, Status, Tag, Target};
use desktop_gpui::screens::terminal::session::{Out, Session};
use desktop_gpui::screens::terminal::words::SHELL_ENDED_LINE;
use gpui_kit::{AppContext, Entity, TestAppContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest, WorkspacesListRequest};
use wsp_host_client::{Event, HostClient};

const WAIT: Duration = Duration::from_secs(30);

/// A claude that answers each prompt "ok" at once: one turn opens the project folder's workspace.
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
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;

struct Host {
    server: Throwaway,
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    /// The workspace of a project folder the host was given, whose channel keeps the thread a shell was opened for.
    workspace: String,
    /// That project folder.
    folder: std::path::PathBuf,
}

impl Host {
    fn start() -> Option<Host> {
        Host::start_on(Arc::new(Runtime::new().unwrap()))
    }

    /// The same, its client on `runtime`, whose scheduler orders an attach's answer against its replay.
    fn start_on(runtime: Arc<Runtime>) -> Option<Host> {
        if let Err(why) = wsp_daemon() {
            println!("skipped: {why}; run apps/desktop-gpui/scripts/stage-daemon.sh");
            return None;
        }
        let server = Throwaway::start_with(ANSWERS, true);
        let folder = server.repo("repo");
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let client = Arc::new(runtime.block_on(server.client()));
        let listed = runtime.block_on(client.call(ask::<WorkspacesListRequest>(json!({})))).unwrap_or_else(|f| panic!("{}", f.message));
        let workspace = listed.workspaces.first().map(|w| w.id.clone()).expect("the folder's workspace");
        Some(Host { server, client, runtime, workspace, folder })
    }

    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    fn target(&self) -> Target {
        Target::Workspace(self.workspace.clone())
    }

    /// Frames down a channel of the test's own to the workspace's daemon, as another window would send them.
    fn daemon(&self, frames: &[Value]) -> Vec<Value> {
        let client = self.client.clone();
        let frames = frames.to_vec();
        let workspace = self.workspace.clone();
        self.runtime.block_on(async move {
            let opened = client
                .call(ask::<DaemonOpenRequest>(json!({ "workspaceId": workspace })))
                .await
                .unwrap_or_else(|f| panic!("{}", f.message));
            let mut replies = Vec::new();
            for frame in frames {
                let sent = client.call(ask::<DaemonSendRequest>(json!({ "channel": opened.channel, "frame": frame.clone() }))).await;
                let reply = sent.unwrap_or_else(|f| panic!("{frame}: {}", f.message)).reply;
                assert_eq!(reply["ok"], true, "{frame}: {reply}");
                replies.push(reply);
            }
            client.call(ask::<DaemonCloseRequest>(json!({ "channel": opened.channel }))).await.unwrap_or_else(|f| panic!("{}", f.message));
            replies
        })
    }

    /// Waits until `pty` has printed `text`, read on a channel of the test's own that attaches it and then lets go.
    fn printed(&self, pty: &str, text: &str) {
        let (client, workspace, pty, text) = (self.client.clone(), self.workspace.clone(), pty.to_owned(), text.to_owned());
        self.runtime.block_on(async move {
            let mut events = client.events().await.unwrap_or_else(|f| panic!("{}", f.message));
            let opened = client
                .call(ask::<DaemonOpenRequest>(json!({ "workspaceId": workspace })))
                .await
                .unwrap_or_else(|f| panic!("{}", f.message));
            let attach = json!({ "channel": opened.channel, "frame": { "op": "pty.attach", "ptyId": pty } });
            client.call(ask::<DaemonSendRequest>(attach)).await.unwrap_or_else(|f| panic!("{}", f.message));
            let mut seen = String::new();
            let read = async {
                while let Some(next) = events.next().await {
                    let Ok(Event::Daemon(pushed)) = next else { continue };
                    let pushed = serde_json::to_value(&pushed).unwrap();
                    if pushed["channel"] == opened.channel.as_str() && pushed["event"]["ptyId"] == pty.as_str() {
                        seen.push_str(pushed["event"]["data"].as_str().unwrap_or_default());
                        if seen.contains(&text) {
                            return;
                        }
                    }
                }
            };
            let found = tokio::time::timeout(WAIT, read).await;
            assert!(found.is_ok(), "{text:?} printed within {} s; printed {seen:?}", WAIT.as_secs());
            client.call(ask::<DaemonCloseRequest>(json!({ "channel": opened.channel }))).await.unwrap_or_else(|f| panic!("{}", f.message));
        });
    }

    fn listed(&self) -> Vec<Value> {
        let replies = self.daemon(&[json!({ "op": "pty.list" })]);
        replies[0]["ptys"].as_array().cloned().unwrap_or_default()
    }
}

/// Lets real time pass, the host's pushes landing on the window's thread, until `done` holds.
fn until(cx: &mut TestAppContext, what: &str, host: &Host, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s\nhost log: {}", WAIT.as_secs(), host.server.log());
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn screen(session: &Entity<Session>, cx: &mut TestAppContext) -> String {
    session.update(cx, |s, _| {
        let mut frame = Frame::default();
        s.engine_mut().frame(&mut frame);
        frame.rows.iter().map(|r| r.plain().trim_end().to_owned()).collect::<Vec<_>>().join("\n")
    })
}

fn shows_line(session: &Entity<Session>, line: &str, cx: &mut TestAppContext) -> bool {
    screen(session, cx).lines().any(|l| l.trim() == line)
}

fn live(link: &Entity<Link>, cx: &mut TestAppContext) -> bool {
    link.read_with(cx, |l, _| *l.status() == Status::Live)
}

fn open_link(host: &Host, cx: &mut TestAppContext) -> Entity<Link> {
    cx.executor().allow_parking();
    let shell = host.shell();
    let target = host.target();
    let link = cx.update(|cx| Link::get(&shell, target, cx));
    until(cx, "the link live", host, |cx| live(&link, cx));
    link
}

#[gpui_kit::test]
fn a_shell_opens_answers_resizes_and_ends_through_the_link(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let shell = host.shell();
    assert_eq!(cx.update(|cx| Link::get(&shell, host.target(), cx)), link, "a second ask hands back the same channel");
    assert_ne!(cx.update(|cx| Link::get(&shell, Target::Here, cx)), link, "this computer's own channel is another");

    let created = link.update(cx, |l, cx| l.create(Tag::Thread("t1".into()), 80, 24, cx));
    until(cx, "the shell created and attached", &host, |cx| link.read_with(cx, |l, _| !l.ptys().is_empty()));
    let session = link.read_with(cx, |l, _| l.ptys()[0].session.clone());
    let pty = session.read_with(cx, |s, _| s.pty().to_string());
    assert_eq!(session.read_with(cx, |s, _| s.title().to_string()), "shell 1");
    assert_eq!(link.read_with(cx, |l, _| l.ptys()[0].tag.clone()), Tag::Thread("t1".into()));

    session.update(cx, |s, cx| s.send(b"echo wsp-$((6*7))\r", cx));
    until(cx, "wsp-42 on the shell's screen", &host, |cx| shows_line(&session, "wsp-42", cx));

    session.update(cx, |s, cx| {
        s.resize(100, 30, 8, 16, cx);
        s.tell_size(cx);
    });
    let row = |host: &Host| host.listed().into_iter().find(|r| r["id"] == pty.as_str());
    until(cx, "the daemon listing the shell at 100 x 30", &host, |_| row(&host).is_some_and(|r| r["cols"] == 100 && r["rows"] == 30));
    assert_eq!(row(&host).unwrap()["thread"], "t1", "the host keeps the thread the shell was opened for");

    let second = link.update(cx, |l, cx| l.create(Tag::None, 0, 0, cx));
    until(cx, "a second shell", &host, |cx| link.read_with(cx, |l, _| l.ptys().len() == 2));
    drop(second);
    let titles: Vec<String> = link.read_with(cx, |l, cx| l.ptys().iter().map(|p| p.session.read(cx).title().to_string()).collect());
    assert_eq!(titles, ["shell 1", "shell 2"]);

    link.update(cx, |l, cx| l.kill(&pty, cx));
    assert_eq!(link.read_with(cx, |l, _| l.ptys().len()), 1, "a killed shell leaves the list at once");
    until(cx, "the daemon no longer listing the killed shell", &host, |_| row(&host).is_none());
    drop(created);
}

#[gpui_kit::test]
fn a_shell_from_elsewhere_is_adopted_and_replays_after_the_channel_drops(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let created = host.daemon(&[json!({ "op": "pty.create", "cols": 80, "rows": 24, "cwd": host.server.dir.path() })]);
    let elsewhere = created[0]["ptyId"].as_str().unwrap().to_owned();
    host.daemon(&[
        json!({ "op": "pty.write", "ptyId": elsewhere, "data": "echo adopted-$((2*3))\r" }),
        json!({ "op": "pty.thread", "ptyId": elsewhere, "thread": "t9" }),
    ]);

    let link = open_link(&host, cx);
    let adopted = link.read_with(cx, |l, cx| l.ptys().iter().find(|p| p.session.read(cx).pty().as_ref() == elsewhere).cloned());
    let adopted = adopted.expect("the shell another window opened is adopted as a tab");
    assert_eq!(adopted.tag, Tag::Thread("t9".into()), "with the thread the host keeps for it");
    let session = adopted.session;
    assert_eq!(session.read_with(cx, |s, _| s.title().to_string()), "shell 1");
    until(cx, "the daemon's scrollback replayed into the adopted shell", &host, |cx| shows_line(&session, "adopted-6", cx));

    let doomed = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    until(cx, "a second shell", &host, |cx| link.read_with(cx, |l, _| l.ptys().len() == 2));
    drop(doomed);
    let doomed = link.read_with(cx, |l, _| l.ptys()[1].session.clone());
    let doomed_pty = doomed.read_with(cx, |s, _| s.pty().to_string());

    link.update(cx, |l, cx| l.drop_for_test(cx));
    assert!(matches!(link.read_with(cx, |l, _| l.status().clone()), Status::Connecting { .. }), "a link live once reconnects");
    // Ended while the channel is down, as a daemon restart ends it: the re-attach finds it gone.
    host.daemon(&[json!({ "op": "pty.kill", "ptyId": doomed_pty })]);
    cx.executor().advance_clock(Duration::from_secs(1));
    until(cx, "the link live again", &host, |cx| live(&link, cx));

    until(cx, "the daemon's scrollback replayed after the reconnect", &host, |cx| shows_line(&session, "adopted-6", cx));
    let seen = screen(&session, cx).lines().filter(|l| l.trim() == "adopted-6").count();
    assert_eq!(seen, 1, "the screen is wiped before the replay, so the output shows once:\n{}", screen(&session, cx));
    until(cx, "the lost shell marked with its sentence", &host, |cx| doomed.read_with(cx, |s, _| s.lost().is_some()));
    assert_eq!(doomed.read_with(cx, |s, _| s.lost().cloned()).unwrap().as_ref(), SHELL_ENDED_LINE);
    assert!(screen(&doomed, cx).contains(&format!("[{SHELL_ENDED_LINE}]")), "{}", screen(&doomed, cx));

    session.update(cx, |s, cx| s.send(b"echo again-$((3*3))\r", cx));
    until(cx, "typing reaching the shell after the reconnect", &host, |cx| shows_line(&session, "again-9", cx));
    assert_eq!(link.read_with(cx, |l, _| l.ptys().len()), 2, "a re-attach adopts nothing twice");
}

#[gpui_kit::test]
fn a_shell_that_exits_says_so_and_takes_no_more_keys(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let created = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    until(cx, "the shell", &host, |cx| link.read_with(cx, |l, _| !l.ptys().is_empty()));
    drop(created);
    let session = link.read_with(cx, |l, _| l.ptys()[0].session.clone());
    until(cx, "the shell's mode reported", &host, |cx| session.read_with(cx, |s, _| s.mode().is_some()));
    session.update(cx, |s, cx| s.send(b"exit 3\r", cx));
    until(cx, "the exit", &host, |cx| session.read_with(cx, |s, _| s.exited().is_some()));
    assert_eq!(session.read_with(cx, |s, _| s.exited()), Some(3));
    assert!(screen(&session, cx).contains("[process exited]"), "{}", screen(&session, cx));
    session.update(cx, |s, cx| s.send(b"x", cx));
    assert!(session.update(cx, |s, _| s.take_out()).is_empty(), "an exited shell takes no keys");
}

#[gpui_kit::test]
fn output_replayed_on_attach_is_not_answered_again(cx: &mut TestAppContext) {
    let session = cx.new(|_| Session::new("p1", "shell 1", 20, 4));
    session.update(cx, |s, cx| {
        s.begin_replay();
        s.feed(b"\x1b[6n", cx);
    });
    assert!(session.update(cx, |s, _| s.take_out()).is_empty(), "a query in the replayed output was answered long ago");
    session.update(cx, |s, cx| {
        s.end_replay(cx);
        s.feed(b"\x1b[6n", cx);
    });
    assert_eq!(session.update(cx, |s, _| s.take_out()), [Out::Write("\x1b[1;1R".into())], "a live query is answered");
}

#[gpui_kit::test]
fn a_shell_opened_while_many_left_behind_are_adopted_takes_its_keys(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let left: Vec<Value> = (0..40).map(|_| json!({ "op": "pty.create", "cols": 80, "rows": 24, "cwd": host.server.dir.path() })).collect();
    let made = host.daemon(&left);
    let tags: Vec<Value> =
        made.iter().enumerate().map(|(i, m)| json!({ "op": "pty.thread", "ptyId": m["ptyId"], "thread": format!("t{}", i % 7) })).collect();
    host.daemon(&tags);

    let link = open_link(&host, cx);
    let opening = link.update(cx, |l, cx| l.create(Tag::Thread("mine".into()), 80, 24, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the new shell opened beside the adoption", &host, |_| opened.borrow().is_some());
    let session = opened.borrow_mut().take().unwrap().unwrap_or_else(|why| panic!("{why}"));
    session.update(cx, |s, cx| s.send(b"echo mine-$((4*5))\r", cx));
    until(cx, "typing reaching the shell opened beside the adoption", &host, |cx| shows_line(&session, "mine-20", cx));
    let ptys: Vec<String> = link.read_with(cx, |l, cx| l.ptys().iter().map(|p| p.session.read(cx).pty().to_string()).collect());
    let mut unique = ptys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ptys.len(), "no pty is held by two shells");
    assert_eq!(ptys.len(), 41, "the forty left behind and the new one");
}

/// Moves the test clock past the heartbeats one at a time, letting each ping come back before the next beat.
fn pass_time(cx: &mut TestAppContext, seconds: u64) {
    for _ in 0..seconds / 10 {
        cx.executor().advance_clock(Duration::from_secs(10));
        let end = Instant::now() + Duration::from_secs(2);
        while Instant::now() < end {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[gpui_kit::test]
fn a_link_nothing_holds_lets_go_and_the_next_finds_its_shells_again(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let opening = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the shell", &host, |_| opened.borrow().is_some());
    let session = opened.borrow_mut().take().unwrap().unwrap_or_else(|why| panic!("{why}"));
    let pty = session.read_with(cx, |s, _| s.pty().to_string());
    session.update(cx, |s, cx| s.send(b"echo kept-$((3*4))\r", cx));
    until(cx, "the shell's output", &host, |cx| shows_line(&session, "kept-12", cx));

    let hold = link.update(cx, |l, _| l.hold());
    pass_time(cx, 50);
    let same = cx.update(|cx| Link::get(&host.shell(), host.target(), cx));
    assert_eq!(same, link, "a held link keeps its channel");

    drop(hold);
    pass_time(cx, 50);
    assert!(link.read_with(cx, |l, _| l.ptys().is_empty()), "a link nothing holds lets its shells' screens go");
    let again = cx.update(|cx| Link::get(&host.shell(), host.target(), cx));
    assert_ne!(again, link, "the next ask dials a new channel");
    until(cx, "the new link live", &host, |cx| live(&again, cx));
    let back = again.read_with(cx, |l, cx| l.ptys().iter().find(|p| p.session.read(cx).pty().as_ref() == pty).map(|p| p.session.clone()));
    let back = back.expect("the daemon's shell adopted again");
    until(cx, "the shell's output replayed", &host, |cx| shows_line(&back, "kept-12", cx));
}

#[gpui_kit::test]
fn a_size_given_while_the_channel_is_down_reaches_the_pty_once_it_is_back(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let opening = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the shell", &host, |_| opened.borrow().is_some());
    let session = opened.borrow_mut().take().unwrap().unwrap_or_else(|why| panic!("{why}"));
    let pty = session.read_with(cx, |s, _| s.pty().to_string());

    link.update(cx, |l, cx| l.drop_for_test(cx));
    session.update(cx, |s, cx| {
        s.resize(90, 20, 8, 16, cx);
        s.tell_size(cx);
    });
    cx.executor().advance_clock(Duration::from_secs(1));
    until(cx, "the link live again", &host, |cx| live(&link, cx));
    until(cx, "the daemon listing the shell at 90 x 20", &host, |_| {
        host.listed().iter().any(|p| p["id"] == pty.as_str() && p["cols"] == 90 && p["rows"] == 20)
    });
}

#[gpui_kit::test]
fn a_new_shells_own_startup_query_is_answered(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    // As fish asks for the terminal's attributes as it starts, before anything has attached to its pty. The daemon runs
    // the passwd row's login shell, zsh on the Mac and bash on a Linux runner, so both read the same lines.
    let rc = "printf '\\033[c'; read -rs -t 5 -d c r; [ -n \"$r\" ] && echo da1-answered\n";
    for file in [".zshrc", ".bash_profile"] {
        std::fs::write(host.server.dir.path().join(file), rc).unwrap();
    }
    let link = open_link(&host, cx);
    let opening = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the shell", &host, |_| opened.borrow().is_some());
    let session = opened.borrow_mut().take().unwrap().unwrap_or_else(|why| panic!("{why}"));
    until(cx, "the shell's query answered", &host, |cx| shows_line(&session, "da1-answered", cx));
}

/// One worker kept busy 3 ms at a time: a replay and its answer are read together, and the answer, woken last, runs first.
fn answer_first() -> Arc<Runtime> {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    runtime.spawn(async {
        loop {
            std::thread::sleep(Duration::from_millis(3));
            tokio::task::yield_now().await;
        }
    });
    runtime
}

/// One thread that runs tasks in the order they were woken, so an attach's replay reaches the window before its answer.
fn replay_first() -> Arc<Runtime> {
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap());
    let driven = runtime.clone();
    std::thread::spawn(move || driven.block_on(std::future::pending::<()>()));
    runtime
}

/// A shell of `/bin/sh` opened on a channel of the test's own and sent `line`, which the link has not seen yet.
fn shell_elsewhere(host: &Host, line: &str) -> String {
    let made = host.daemon(&[json!({ "op": "pty.create", "cols": 80, "rows": 60, "cwd": host.server.dir.path(), "shell": "/bin/sh" })]);
    let pty = made[0]["ptyId"].as_str().unwrap().to_owned();
    host.daemon(&[json!({ "op": "pty.write", "ptyId": pty, "data": line })]);
    pty
}

fn adopted(link: &Entity<Link>, pty: &str, cx: &mut TestAppContext) -> Entity<Session> {
    let held = link.read_with(cx, |l, cx| l.ptys().iter().find(|p| p.session.read(cx).pty().as_ref() == pty).map(|p| p.session.clone()));
    held.expect("the shell adopted")
}

fn opened_shell(link: &Entity<Link>, cols: u16, rows: u16, host: &Host, cx: &mut TestAppContext) -> Entity<Session> {
    let opening = link.update(cx, |l, cx| l.create(Tag::None, cols, rows, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the shell", host, |_| opened.borrow().is_some());
    let created = opened.borrow_mut().take().unwrap();
    created.unwrap_or_else(|why| panic!("{why}"))
}

#[gpui_kit::test]
fn a_query_in_the_replay_is_not_answered_again_when_the_attach_answer_comes_first(cx: &mut TestAppContext) {
    let Some(host) = Host::start_on(answer_first()) else { return };
    // It asks for the cursor with nothing attached, then echoes input; the split quote keeps the echo from reading "asked".
    let pty = shell_elsewhere(&host, "seq 1 3000; printf 'ask''ed\\033[6n'; exec cat -v\r");
    host.printed(&pty, "asked");
    let link = open_link(&host, cx);
    let session = adopted(&link, &pty, cx);
    for round in 0..20 {
        if round > 0 {
            link.update(cx, |l, cx| l.drop_for_test(cx));
            cx.executor().advance_clock(Duration::from_secs(1));
            until(cx, "the link live again", &host, |cx| live(&link, cx));
        }
        until(cx, "the replayed output", &host, |cx| screen(&session, cx).lines().any(|l| l.starts_with("asked")));
        let mark = format!("mark-{round}");
        session.update(cx, |s, cx| s.send(format!("{mark}\r").as_bytes(), cx));
        until(cx, "the mark shown back", &host, |cx| screen(&session, cx).lines().filter(|l| l.ends_with(mark.as_str())).count() == 2);
        let shown = screen(&session, cx);
        assert!(!shown.contains("^["), "replay {round}: the query it carried was answered again:\n{shown}");
    }
}

#[gpui_kit::test]
fn an_adopted_shells_replay_reaches_its_screen_when_it_comes_before_the_attach_answer(cx: &mut TestAppContext) {
    let Some(host) = Host::start_on(replay_first()) else { return };
    let pty = shell_elsewhere(&host, "echo early-$((2*4))\r");
    host.printed(&pty, "early-8");
    let link = open_link(&host, cx);
    let session = adopted(&link, &pty, cx);
    until(cx, "the replay on the adopted shell's screen", &host, |cx| shows_line(&session, "early-8", cx));
}

#[gpui_kit::test]
fn a_shell_closed_while_the_channel_is_down_ends_once_it_is_back(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let session = opened_shell(&link, 80, 24, &host, cx);
    let pty = session.read_with(cx, |s, _| s.pty().to_string());
    link.update(cx, |l, cx| l.drop_for_test(cx));
    link.update(cx, |l, cx| l.kill(&pty, cx));
    assert!(link.read_with(cx, |l, _| l.ptys().is_empty()), "the tab goes at once");
    cx.executor().advance_clock(Duration::from_secs(1));
    until(cx, "the link live again", &host, |cx| live(&link, cx));
    assert!(link.read_with(cx, |l, _| l.ptys().is_empty()), "the closed shell is not adopted back");
    until(cx, "the daemon no longer listing the closed shell", &host, |_| !host.listed().iter().any(|r| r["id"] == pty.as_str()));
}

#[gpui_kit::test]
fn output_written_across_reconnects_shows_once_and_whole(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let session = opened_shell(&link, 80, 40, &host, cx);
    session.update(cx, |s, cx| s.send(b"i=1; while [ $i -le 30 ]; do echo n$i; i=$((i+1)); sleep 0.1; done\r", cx));
    for at in ["n5", "n15"] {
        until(cx, at, &host, |cx| shows_line(&session, at, cx));
        link.update(cx, |l, cx| l.drop_for_test(cx));
        // The shell goes on printing with no channel up.
        std::thread::sleep(Duration::from_millis(600));
        cx.executor().advance_clock(Duration::from_secs(1));
        until(cx, "the link live again", &host, |cx| live(&link, cx));
    }
    until(cx, "the last line", &host, |cx| shows_line(&session, "n30", cx));
    let shown = screen(&session, cx);
    for n in 1..=30 {
        let seen = shown.lines().filter(|l| l.trim() == format!("n{n}")).count();
        assert_eq!(seen, 1, "n{n} shows {seen} times:\n{shown}");
    }
}

#[gpui_kit::test]
fn a_link_held_again_within_its_release_keeps_its_channel(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let hold = link.update(cx, |l, _| l.hold());
    opened_shell(&link, 80, 24, &host, cx);
    let same = |cx: &mut TestAppContext| cx.update(|cx| Link::get(&host.shell(), host.target(), cx)) == link;
    drop(hold);
    pass_time(cx, 30);
    let hold = link.update(cx, |l, _| l.hold());
    pass_time(cx, 50);
    assert!(same(cx), "held again within 30 s, the link keeps its channel");
    assert_eq!(link.read_with(cx, |l, _| l.ptys().len()), 1);
    drop(hold);
    pass_time(cx, 20);
    assert!(same(cx), "the release counts from the last let-go");
    pass_time(cx, 20);
    assert!(!same(cx), "and comes once 30 s have passed unheld");
}

#[gpui_kit::test]
fn two_windows_on_one_shell_both_see_it_both_type_and_one_letting_go_keeps_it(cx: &mut TestAppContext, other: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let _held = link.update(cx, |l, _| l.hold());
    let mine = opened_shell(&link, 80, 24, &host, cx);
    let pty = mine.read_with(cx, |s, _| s.pty().to_string());

    // Another window is another app, on a socket of its own.
    let client = Arc::new(host.runtime.block_on(host.server.client()));
    let shell = desktop_gpui::screens::Host::new(client, host.runtime.clone());
    let theirs = other.update(|cx| Link::get(&shell, host.target(), cx));
    until(other, "the other window's link live", &host, |cx| live(&theirs, cx));
    let seen = adopted(&theirs, &pty, other);

    mine.update(cx, |s, cx| s.send(b"echo from-$((1+1))\r", cx));
    until(cx, "this window's own typing", &host, |cx| shows_line(&mine, "from-2", cx));
    until(other, "this window's typing in the other", &host, |cx| shows_line(&seen, "from-2", cx));
    seen.update(other, |s, cx| s.send(b"echo two-$((1+2))\r", cx));
    until(other, "the other window's own typing", &host, |cx| shows_line(&seen, "two-3", cx));
    until(cx, "the other window's typing in this one", &host, |cx| shows_line(&mine, "two-3", cx));

    let size = |host: &Host| host.listed().into_iter().find(|r| r["id"] == pty.as_str()).map(|r| (r["cols"].clone(), r["rows"].clone()));
    mine.update(cx, |s, cx| {
        s.resize(100, 30, 8, 16, cx);
        s.tell_size(cx);
    });
    until(cx, "this window's size", &host, |_| size(&host) == Some((100.into(), 30.into())));
    seen.update(other, |s, cx| {
        s.resize(90, 20, 8, 16, cx);
        s.tell_size(cx);
    });
    until(other, "the size told last", &host, |_| size(&host) == Some((90.into(), 20.into())));

    pass_time(other, 50);
    assert!(theirs.read_with(other, |l, _| l.ptys().is_empty()), "the other window let its channel go");
    assert!(live(&link, cx), "this window's channel stays");
    assert!(size(&host).is_some(), "the shell outlives the other window's channel");
    mine.update(cx, |s, cx| s.send(b"echo still-$((2*2))\r", cx));
    until(cx, "typing after the other window let go", &host, |cx| shows_line(&mine, "still-4", cx));
}

#[gpui_kit::test]
fn a_shell_opened_while_the_reconnect_re_attaches_is_held_once(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let held = opened_shell(&link, 80, 24, &host, cx);
    // A long scrollback keeps the held shell's re-attach out for a while.
    held.update(cx, |s, cx| s.send(b"seq 1 30000; echo held-$((5*5))\r", cx));
    until(cx, "the held shell's output", &host, |cx| shows_line(&held, "held-25", cx));
    link.update(cx, |l, cx| l.drop_for_test(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    // Asked until the channel takes it: then the held shell's re-attach is out, and the listing after it is not yet.
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    loop {
        cx.run_until_parked();
        let opening = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
        let into = opened.clone();
        cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
        cx.run_until_parked();
        if !matches!(&*opened.borrow(), Some(Err(why)) if why.as_ref() == "not answering") {
            break;
        }
        *opened.borrow_mut() = None;
        std::thread::sleep(Duration::from_millis(1));
    }
    until(cx, "the link live again", &host, |cx| live(&link, cx));
    until(cx, "the shell opened during the reconnect", &host, |_| opened.borrow().is_some());
    let created = opened.borrow_mut().take().unwrap();
    let new = created.unwrap_or_else(|why| panic!("{why}")).read_with(cx, |s, _| s.pty().to_string());
    let ptys: Vec<String> = link.read_with(cx, |l, cx| l.ptys().iter().map(|p| p.session.read(cx).pty().to_string()).collect());
    assert_eq!(ptys.iter().filter(|p| **p == new).count(), 1, "one session holds the new shell: {ptys:?}");
    assert_eq!(ptys.len(), 2, "{ptys:?}");
}

#[gpui_kit::test]
fn a_redial_after_a_nap_opens_the_channel_at_once(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    link.update(cx, |l, cx| l.drop_for_test(cx));
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert!(!live(&link, cx), "the backoff is still being waited out");
    link.update(cx, |l, cx| l.redial(cx));
    until(cx, "the link live with the clock short of its backoff", &host, |cx| live(&link, cx));
}

#[gpui_kit::test]
fn an_adopted_shell_awaits_its_replay_until_the_replay_is_on_its_screen(cx: &mut TestAppContext) {
    let Some(host) = Host::start_on(answer_first()) else { return };
    let pty = shell_elsewhere(&host, "seq 1 3000; echo replayed-$((3*5))\r");
    host.printed(&pty, "replayed-15");
    cx.executor().allow_parking();
    let link = cx.update(|cx| Link::get(&host.shell(), host.target(), cx));
    // (awaiting its replay, the replay's last line on the screen) at the listing and at every change after it.
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(bool, bool)>::new()));
    let watching = std::rc::Rc::new(std::cell::RefCell::new(None));
    let (into, held, watched) = (seen.clone(), watching.clone(), pty.clone());
    let _listed = cx.update(|cx| {
        cx.subscribe(&link, move |link, _, cx| {
            if held.borrow().is_some() {
                return;
            }
            let found = link.read(cx).ptys().iter().find(|p| p.session.read(cx).pty().as_ref() == watched).map(|p| p.session.clone());
            let Some(session) = found else { return };
            let note = |session: &Entity<Session>, into: &std::cell::RefCell<Vec<(bool, bool)>>, cx: &mut gpui_kit::App| {
                let mut frame = Frame::default();
                let shown = session.update(cx, |s, _| {
                    s.engine_mut().frame(&mut frame);
                    frame.rows.iter().any(|r| r.plain().trim() == "replayed-15")
                });
                into.borrow_mut().push((session.read(cx).awaiting_replay(), shown));
            };
            note(&session, &into, cx);
            let into = into.clone();
            *held.borrow_mut() = Some(cx.observe(&session, move |session, cx| note(&session, &into, cx)));
        })
    });
    until(cx, "the shell adopted", &host, |_| !seen.borrow().is_empty());
    let session = adopted(&link, &pty, cx);
    until(cx, "the replay over", &host, |cx| !session.read_with(cx, |s, _| s.awaiting_replay()));
    let seen = seen.borrow().clone();
    assert!(seen[0].0, "a shell is listed while it still awaits its replay: {seen:?}");
    assert!(seen.iter().all(|&(awaiting, shown)| awaiting || shown), "it awaits until the replay is on its screen: {seen:?}");
    assert!(shows_line(&session, "replayed-15", cx));
    let made = opened_shell(&link, 80, 24, &host, cx);
    assert!(!made.read_with(cx, |s, _| s.awaiting_replay()), "a shell this link created has nothing to replay");
}

#[gpui_kit::test]
fn a_link_let_go_before_its_open_reaches_it_closes_the_channel_on_the_host(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let mut events = host.runtime.block_on(host.client.events()).unwrap_or_else(|f| panic!("{}", f.message));
    let link = cx.update(|cx| Link::get(&host.shell(), host.target(), cx));
    // The window's tasks do not run here, so the host's answer to the open waits on its way to the link.
    let channel = host.runtime.block_on(async {
        let first = async {
            while let Some(next) = events.next().await {
                let Ok(Event::Daemon(pushed)) = next else { continue };
                return serde_json::to_value(&pushed).unwrap()["channel"].as_str().map(str::to_owned);
            }
            None
        };
        tokio::time::timeout(WAIT, first).await.ok().flatten()
    });
    let channel = channel.expect("the host pushing on the channel it opened");
    link.update(cx, |l, cx| l.release_for_test(cx));
    let open = |host: &Host| {
        let ping = json!({ "channel": channel, "frame": { "op": "ping" } });
        host.runtime.block_on(host.client.call(ask::<DaemonSendRequest>(ping))).is_ok()
    };
    until(cx, "the channel closed on the host", &host, |_| !open(&host));
}

#[gpui_kit::test]
fn a_link_let_go_while_its_open_is_out_closes_the_channel_the_host_opens_after(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let mut events = host.runtime.block_on(host.client.events()).unwrap_or_else(|f| panic!("{}", f.message));
    let channel_of = |next: Option<Result<Event, wsp_host_client::Failure>>| match next {
        Some(Ok(Event::Daemon(pushed))) => serde_json::to_value(&pushed).unwrap()["channel"].as_str().map(str::to_owned),
        _ => None,
    };
    let open = |host: &Host, channel: &str| {
        let ping = json!({ "channel": channel, "frame": { "op": "ping" } });
        host.runtime.block_on(host.client.call(ask::<DaemonSendRequest>(ping))).is_ok()
    };
    // Let go after a wait that grows each round, until a round lets go before the host has answered the open.
    let mut known = std::collections::HashSet::new();
    let mut caught = None;
    for wait in [0, 1, 2, 3, 5, 8, 12, 20, 30, 50] {
        let link = cx.update(|cx| Link::get(&host.shell(), host.target(), cx));
        std::thread::sleep(Duration::from_millis(wait));
        let ready: Vec<String> =
            host.runtime.block_on(async { std::iter::from_fn(|| events.next().now_or_never()).filter_map(&channel_of).collect() });
        let answered: Vec<String> = ready.into_iter().filter(|c| known.insert(c.clone())).collect();
        link.update(cx, |l, cx| l.release_for_test(cx));
        cx.run_until_parked();
        let later = host.runtime.block_on(async {
            let first = async {
                loop {
                    if let Some(channel) = channel_of(events.next().await).filter(|c| !known.contains(c)) {
                        return channel;
                    }
                }
            };
            tokio::time::timeout(Duration::from_secs(2), first).await.ok()
        });
        known.extend(later.clone());
        for channel in answered.iter().chain(later.iter()) {
            until(cx, "the channel closed on the host", &host, |_| !open(&host, channel));
        }
        if answered.is_empty() && later.is_some() {
            caught = later;
            break;
        }
    }
    assert!(caught.is_some(), "no round let go while the open was out");
}

#[gpui_kit::test]
fn a_watched_computers_last_reading_is_kept_once_the_channel_drops(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    assert_eq!(link.read_with(cx, |l, _| l.memory()), None, "nothing is read before a watch");
    link.update(cx, |l, cx| l.watch_sys(cx));
    until(cx, "a reading", &host, |cx| link.read_with(cx, |l, _| l.memory()).is_some());
    let read = link.read_with(cx, |l, _| l.memory()).unwrap();
    assert!(read.total > 0. && read.used > 0. && read.used <= read.total, "{read:?}");
    link.update(cx, |l, cx| l.drop_for_test(cx));
    assert_eq!(link.read_with(cx, |l, _| l.memory()), Some(read), "the reading outlives the channel");
}

#[gpui_kit::test]
fn a_port_watch_says_what_listens_in_the_workspace_and_when_it_stops(cx: &mut TestAppContext) {
    use desktop_gpui::screens::terminal::link::LinkEvent;
    use std::io::BufRead as _;
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let said: std::rc::Rc<std::cell::RefCell<Vec<Value>>> = Default::default();
    let heard = said.clone();
    let _watch = cx.update(|cx| {
        cx.subscribe(&link, move |_, event: &LinkEvent, _| match event {
            LinkEvent::Ports(rows) => heard.borrow_mut().extend(rows.as_array().cloned().unwrap_or_default()),
            LinkEvent::Port(event) => heard.borrow_mut().push(event.clone()),
            LinkEvent::Changed => {}
        })
    });
    // A listener whose folder is the workspace's, as a dev server started in it is.
    let folder = host.folder.clone();
    /// The listener, stopped however the test ends.
    struct Listener(std::process::Child);
    impl Drop for Listener {
        fn drop(&mut self) {
            _ = self.0.kill();
            _ = self.0.wait();
        }
    }
    let mut server = std::process::Command::new("python3")
        .args(["-c", "import socket, time\ns = socket.socket()\ns.bind(('127.0.0.1', 0))\ns.listen()\nprint(s.getsockname()[1], flush=True)\ntime.sleep(120)"])
        .current_dir(&folder)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("python3 listens");
    let out = server.stdout.take().unwrap();
    let server = Listener(server);
    let mut line = String::new();
    std::io::BufReader::new(out).read_line(&mut line).unwrap();
    let port: u64 = line.trim().parse().expect("the listener says its port");
    link.update(cx, |l, cx| l.watch_ports(cx));
    let lists = |said: &[Value]| said.iter().any(|p| p["port"] == port && p["type"] != "port.close");
    until(cx, "the port listed", &host, |_| lists(&said.borrow()));
    drop(server);
    let closed = |said: &[Value]| said.iter().any(|p| p["type"] == "port.close" && p["port"] == port);
    until(cx, "the port's close", &host, |_| closed(&said.borrow()));
}

#[gpui_kit::test]
fn each_port_watch_lists_the_ports_again(cx: &mut TestAppContext) {
    use desktop_gpui::screens::terminal::link::LinkEvent;
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let lists = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = lists.clone();
    let _watch = cx.update(|cx| {
        cx.subscribe(&link, move |_, event: &LinkEvent, _| {
            if let LinkEvent::Ports(_) = event {
                counted.set(counted.get() + 1);
            }
        })
    });
    link.update(cx, |l, cx| l.watch_ports(cx));
    until(cx, "the first list", &host, |_| lists.get() == 1);
    // A browser screen built again missed the port events from while none was up, so it is told the whole list again.
    link.update(cx, |l, cx| l.watch_ports(cx));
    until(cx, "the second list", &host, |_| lists.get() == 2);
}

/// A window drawing a shell that sends it one line from inside its second draw, once the window tracks the shell.
struct SendsWhileDrawn {
    session: Entity<Session>,
    draws: usize,
    sent: usize,
}

impl gpui_kit::Render for SendsWhileDrawn {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        let title = self.session.read(cx).title().clone();
        self.draws += 1;
        if self.draws == 2 {
            self.session.update(cx, |s, cx| s.send(b"echo drawn-$((6*7))\r", cx));
            self.sent += 1;
        }
        gpui_kit::ParentElement::child(gpui_kit::div(), title)
    }
}

#[gpui_kit::test]
fn a_line_sent_while_a_window_draws_its_shell_reaches_the_pty(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let link = open_link(&host, cx);
    let session = opened_shell(&link, 80, 24, &host, cx);
    let window = cx.add_window(|_, _| SendsWhileDrawn { session: session.clone(), draws: 0, sent: 0 });
    cx.run_until_parked();
    _ = window.update(cx, |_, window, cx| {
        cx.notify();
        window.refresh();
    });
    cx.run_until_parked();
    // The shell's own output after the send, its prompt or a title, can draw the window again on a slow runner.
    let (draws, sent) = window.update(cx, |v, _, _| (v.draws, v.sent)).unwrap();
    assert!(draws >= 2, "the window drew twice, drew {draws}");
    assert_eq!(sent, 1, "the line went once, from the second draw");
    assert_eq!(session.update(cx, |s, _| s.take_out()), [], "the link took the line as soon as the draw was done");
    until(cx, "the line sent mid-draw run by the shell", &host, |cx| shows_line(&session, "drawn-42", cx));
}
