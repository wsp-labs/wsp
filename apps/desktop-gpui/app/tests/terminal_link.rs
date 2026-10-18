// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal's daemon channel against a host started for each test alone, carrying the wsp-daemon
//! scripts/stage-daemon.sh staged: shells opened, typed into, resized, adopted, re-attached after a drop and ended
//! through `Link`, each checked against what the daemon itself lists. Skips, saying why, where no daemon is staged.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{ask, wsp_daemon, Throwaway};
use desktop_gpui::screens::terminal::engine::Frame;
use desktop_gpui::screens::terminal::link::{Link, Status, Tag, Target};
use desktop_gpui::screens::terminal::session::{Out, Session};
use desktop_gpui::screens::terminal::words::SHELL_ENDED_LINE;
use gpui_kit::{AppContext, Entity, TestAppContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest, WorkspacesListRequest};
use wsp_host_client::HostClient;

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
}

impl Host {
    fn start() -> Option<Host> {
        if let Err(why) = wsp_daemon() {
            println!("skipped: {why}; run apps/desktop-gpui/scripts/stage-daemon.sh");
            return None;
        }
        let server = Throwaway::start_with(ANSWERS, true);
        let folder = server.repo("repo");
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(Runtime::new().unwrap());
        let client = Arc::new(runtime.block_on(server.client()));
        let listed = runtime.block_on(client.call(ask::<WorkspacesListRequest>(json!({})))).unwrap_or_else(|f| panic!("{}", f.message));
        let workspace = listed.workspaces.first().map(|w| w.id.clone()).expect("the folder's workspace");
        Some(Host { server, client, runtime, workspace })
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
        s.end_replay();
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
    // As fish asks for the terminal's attributes as it starts, before anything has attached to its pty.
    let rc = "printf '\\033[c'; read -rs -t 5 -d c r; [ -n \"$r\" ] && echo da1-answered\n";
    std::fs::write(host.server.dir.path().join(".zshrc"), rc).unwrap();
    let link = open_link(&host, cx);
    let opening = link.update(cx, |l, cx| l.create(Tag::None, 80, 24, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let into = opened.clone();
    cx.spawn(async move |_| *into.borrow_mut() = Some(opening.await)).detach();
    until(cx, "the shell", &host, |_| opened.borrow().is_some());
    let session = opened.borrow_mut().take().unwrap().unwrap_or_else(|why| panic!("{why}"));
    until(cx, "the shell's query answered", &host, |cx| shows_line(&session, "da1-answered", cx));
}
