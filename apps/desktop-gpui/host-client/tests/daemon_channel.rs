// SPDX-License-Identifier: AGPL-3.0-only
//! A shell through a real wsp-daemon, against a host started for this test alone and carrying the daemon
//! scripts/stage-daemon.sh staged: the frames the terminal pane sends down its channel, and the bytes that come back.
//! Skips, saying why, where no daemon is staged.

mod common;

use std::time::Duration;

use common::{ask, wsp_daemon, Throwaway};
use serde_json::{json, Value};
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest};
use wsp_host_client::{Event, Events, HostClient};

/// No turn runs here, so the host's claude only has to exist.
const NO_AGENT: &str = "#!/bin/sh\nexit 1\n";

async fn send(client: &HostClient, channel: &str, frame: Value) -> Value {
    let sent = client.call(ask::<DaemonSendRequest>(json!({ "channel": channel, "frame": frame.clone() }))).await;
    let reply = sent.unwrap_or_else(|f| panic!("{frame}: {}", f.message)).reply;
    assert_eq!(reply["ok"], true, "{frame}: {reply}");
    reply
}

/// The channel's pushes, until `done` says enough or 20 s pass; each pushed daemon frame as the host relayed it.
async fn pushed_until(events: &mut Events, channel: &str, mut done: impl FnMut(&Value) -> bool) -> Vec<Value> {
    let mut seen = Vec::new();
    let read = async {
        while let Some(next) = events.next().await {
            let Event::Daemon(pushed) = next.unwrap_or_else(|f| panic!("{}", f.message)) else { continue };
            let pushed = serde_json::to_value(&pushed).unwrap();
            if pushed["channel"] != channel {
                continue;
            }
            assert_eq!(pushed["type"], "daemon.event", "the channel closed: {pushed}");
            seen.push(pushed["event"].clone());
            if done(&pushed["event"]) {
                return;
            }
        }
        panic!("the socket closed");
    };
    if tokio::time::timeout(Duration::from_secs(20), read).await.is_err() {
        panic!("not within 20 s; pushed: {seen:#?}");
    }
    seen
}

#[tokio::test]
async fn a_shell_answers_through_the_daemon_channel() {
    if let Err(why) = wsp_daemon() {
        println!("skipped: {why}; run apps/desktop-gpui/scripts/stage-daemon.sh");
        return;
    }
    let host = Throwaway::start_with(NO_AGENT, true);
    let client = host.client().await;
    // Before the open: the host pushes a channel's frames only to the socket that opened it, from then on.
    let mut events = client.events().await.unwrap_or_else(|f| panic!("{}", f.message));

    let opened = client.call(ask::<DaemonOpenRequest>(json!({ "placeId": "here" }))).await;
    let channel = opened.unwrap_or_else(|f| panic!("daemon.open: {}\nhost log: {}", f.message, host.log())).channel;
    send(&client, &channel, json!({ "op": "ping" })).await;

    let created = send(&client, &channel, json!({ "op": "pty.create", "cols": 80, "rows": 24, "cwd": host.dir.path() })).await;
    let pty = created["ptyId"].as_str().expect("a ptyId").to_owned();
    assert!(created["pid"].as_u64().is_some_and(|pid| pid > 0), "{created}");
    let attached = send(&client, &channel, json!({ "op": "pty.attach", "ptyId": pty })).await;
    assert_eq!(attached["ptyId"], pty.as_str());
    send(&client, &channel, json!({ "op": "pty.resize", "ptyId": pty, "cols": 100, "rows": 30 })).await;

    send(&client, &channel, json!({ "op": "pty.write", "ptyId": pty, "data": "echo wsp-$((6*7))\r" })).await;
    let mut printed = String::new();
    let seen = pushed_until(&mut events, &channel, |event| {
        if event["type"] == "pty.data" && event["ptyId"] == pty.as_str() {
            printed.push_str(event["data"].as_str().unwrap_or_default());
        }
        printed.contains("wsp-42\r\n")
    })
    .await;
    println!("pushed {} frames, types {:?}", seen.len(), seen.iter().map(|e| e["type"].as_str().unwrap_or("?")).collect::<Vec<_>>());

    let listed = send(&client, &channel, json!({ "op": "pty.list" })).await;
    let row = listed["ptys"].as_array().unwrap().iter().find(|row| row["id"] == pty.as_str()).cloned().expect("the pty listed");
    assert_eq!((row["cols"].as_u64(), row["rows"].as_u64(), row["exited"].as_bool()), (Some(100), Some(30), Some(false)), "{row}");

    send(&client, &channel, json!({ "op": "pty.write", "ptyId": pty, "data": "exit 3\r" })).await;
    let exited = pushed_until(&mut events, &channel, |event| event["type"] == "pty.exit" && event["ptyId"] == pty.as_str()).await;
    assert_eq!(exited.last().unwrap()["exitCode"], 3, "{exited:?}");

    send(&client, &channel, json!({ "op": "pty.kill", "ptyId": pty })).await;
    client.call(ask::<DaemonCloseRequest>(json!({ "channel": channel }))).await.unwrap_or_else(|f| panic!("{}", f.message));
}
