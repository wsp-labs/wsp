// SPDX-License-Identifier: AGPL-3.0-only
//! What a thread switch asks the host, counted on the wire: the thread screen as the frame mounts it, against a host
//! started for this test alone and reached through a proxy that reads every frame the app sends. Once each thread of
//! the workspace was opened, the composer's side of a switch asks nothing: no `agents.read`, which runs the agents' own
//! command lines on the host, no `preferences.get`, and no workspace `sessions.list`.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::composer::{self};
use desktop_gpui::screens::thread_view::{header, ThreadScreen};
use desktop_gpui::screens::{self, Opened, THREAD};
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, TestAppContext, VisualTestContext};
use serde_json::Value;
use wsp_host_client::HostClient;

/// Every op the app sent through the proxy, with whether it named a workspace.
type Sent = Arc<Mutex<Vec<(String, bool)>>>;

/// One client frame's op off a websocket stream: the frames after the upgrade, unmasked.
fn frames(stream: &mut Vec<u8>, sent: &Sent) {
    loop {
        if stream.len() < 2 {
            return;
        }
        let (masked, mut len, mut at) = (stream[1] & 0x80 != 0, (stream[1] & 0x7f) as usize, 2);
        if len == 126 {
            if stream.len() < 4 {
                return;
            }
            len = u16::from_be_bytes([stream[2], stream[3]]) as usize;
            at = 4;
        } else if len == 127 {
            if stream.len() < 10 {
                return;
            }
            len = u64::from_be_bytes(stream[2..10].try_into().unwrap()) as usize;
            at = 10;
        }
        let key_at = at;
        if masked {
            at += 4;
        }
        if stream.len() < at + len {
            return;
        }
        let mut payload = stream[at..at + len].to_vec();
        if masked {
            let key = &stream[key_at..key_at + 4];
            payload.iter_mut().enumerate().for_each(|(i, b)| *b ^= key[i % 4]);
        }
        if let Ok(frame) = serde_json::from_slice::<Value>(&payload) {
            if let Some(op) = frame.get("op").and_then(Value::as_str) {
                sent.lock().unwrap().push((op.to_owned(), frame.get("workspaceId").is_some()));
            }
        }
        stream.drain(..at + len);
    }
}

/// A listener that hands each connection to the host on `port` and reads what the app sends; answers its port.
fn proxy(port: u16, sent: Sent) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mine = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let Ok(host) = TcpStream::connect(("127.0.0.1", port)) else { continue };
            let (mut from_host, mut to_client) = (host.try_clone().unwrap(), client.try_clone().unwrap());
            std::thread::spawn(move || _ = std::io::copy(&mut from_host, &mut to_client));
            let (mut from_client, mut to_host, sent) = (client, host, sent.clone());
            std::thread::spawn(move || {
                let (mut buf, mut seen, mut upgraded) = ([0u8; 65536], Vec::new(), false);
                while let Ok(n) = from_client.read(&mut buf) {
                    if n == 0 || to_host.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    if !upgraded {
                        let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                        seen.drain(..end + 4);
                        upgraded = true;
                    }
                    frames(&mut seen, &sent);
                }
            });
        }
    });
    mine
}

/// A state folder beside a copy of the host's, whose lock names the proxy's port: the app dials the proxy.
fn through(host: &Throwaway, port: u16) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let from = host.state().parent().unwrap().to_owned();
    for entry in std::fs::read_dir(&from).unwrap().flatten() {
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), dir.path().join(entry.file_name())).unwrap();
        }
    }
    let lock = dir.path().join("host.lock");
    let mut held: Value = serde_json::from_str(&std::fs::read_to_string(&lock).unwrap()).unwrap();
    held["port"] = Value::from(port);
    std::fs::write(&lock, held.to_string()).unwrap();
    let state = dir.path().join("state.json");
    (dir, state)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(40);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn settle(cx: &mut VisualTestContext) {
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn a_switch_asks_the_host_nothing_for_the_composer_once_each_thread_was_open(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    for prompt in ["short-1", "short-2", "short-3"] {
        host.wsp(&["run", "repo", prompt, "--agent", "claude"]);
    }
    let lock: Value = serde_json::from_str(&std::fs::read_to_string(host.state().with_file_name("host.lock")).unwrap()).unwrap();
    let sent: Sent = Arc::default();
    let port = proxy(lock["port"].as_u64().unwrap() as u16, sent.clone());
    let (_dir, state) = through(&host, port);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&state, &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let id = |prompt: &str| threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).and_then(|t| t.thread_id.clone()).unwrap();
    let workspace = threads[0].workspace_id.clone();
    let order = ["short-1", "short-2", "short-3"].map(id);
    let shell = screens::Host::new(Arc::new(client), runtime.clone());

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let tally = |ops: &[(String, bool)]| {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for (op, named) in ops {
            *by.entry(if *named { format!("{op} (workspace)") } else { op.clone() }).or_default() += 1;
        }
        by
    };
    let switches = 6;

    // The baseline: the thread view alone, with the header's read on each load, as the screen has them less its composer.
    let baseline = {
        let view = cx.new(ThreadView::new);
        let (_, base) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        base.simulate_resize(size(px(1100.), px(800.)));
        base.update(|_, cx| {
            cx.subscribe(&view, |view, event: &ThreadViewEvent, cx| {
                if matches!(event, ThreadViewEvent::Loaded) {
                    view.update(cx, header::refresh);
                }
            })
            .detach()
        });
        let follow = |thread: &str, base: &mut VisualTestContext| {
            let item =
                ThreadItem { workspace_id: workspace.clone(), thread_id: thread.to_owned(), project: String::new(), title: String::new() };
            view.update(base, |v, cx| v.follow(&shell, &item, false, cx));
            until(base, "the thread loaded", |cx| view.read_with(cx, |v, _| v.thread() == Some(thread) && v.loaded()));
            settle(base);
        };
        for thread in &order {
            follow(thread, base);
        }
        sent.lock().unwrap().clear();
        for at in 0..switches {
            follow(&order[at % 3], base);
        }
        let ops: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
        view.update(base, |v, cx| v.stop(cx));
        settle(base);
        sent.lock().unwrap().clear();
        tally(&ops)
    };
    println!("{switches} switches of the view and header alone: {baseline:?}");

    let entry = screens::screen(THREAD).unwrap();
    let opened = |thread: &str| Opened { workspace_id: Some(workspace.clone()), thread_id: Some(thread.to_owned()), ..Opened::default() };
    let mut mounted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = (entry.mount)(&shell, opened(&order[0]), window, cx);
        let view = screen.view.clone();
        mounted = Some(screen);
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1100.), px(800.)));
    let mounted = mounted.unwrap();
    let screen = mounted.view.clone().downcast::<ThreadScreen>().ok().unwrap();
    let (view, composer) = screen.read_with(cx, |s, _| (s.view().clone(), s.composer().clone()));
    let open = |thread: &str, cx: &mut VisualTestContext| {
        cx.update(|window, cx| (mounted.show)(opened(thread), window, cx));
        until(cx, "the thread loaded and its composer linked", |cx| {
            view.read_with(cx, |v, _| v.thread() == Some(thread) && v.loaded())
                && composer.read_with(cx, |c, _| c.thread_key() == Some(thread) && c.hold() != Some(&Hold::Connecting))
        });
        settle(cx);
    };
    for thread in &order {
        open(thread, cx);
    }
    let first: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
    println!("opening the three threads the first time: {:?}", tally(&first));
    assert_eq!(first.iter().filter(|(op, _)| op == "agents.read").count(), 1, "the skills are read once for the workspace's agent");
    assert_eq!(first.iter().filter(|(op, _)| op == "preferences.get").count(), 1, "midTurn is read once");

    for at in 0..switches {
        open(&order[at % 3], cx);
    }
    let after: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
    println!("{switches} switches among them: {:?}", tally(&after));
    let count = |op: &str, named: bool| after.iter().filter(|(o, n)| o == op && *n == named).count();
    let any = |op: &str| after.iter().filter(|(o, _)| o == op).count();
    assert_eq!(any("agents.read"), 0, "no agent probe on a switch");
    assert_eq!(any("preferences.get"), 0, "no preference read on a switch");
    let was = |op: &str| baseline.get(op).copied().unwrap_or(0);
    assert_eq!(count("sessions.list", true), was("sessions.list (workspace)"), "the composer read the workspace's rows on a switch");
    assert_eq!(count("harnesses.list", true), was("harnesses.list (workspace)"), "the composer read the agents on a switch");
    assert_eq!(count("sessions.list", false), was("sessions.list"), "the composer read the threads on a switch");
}
