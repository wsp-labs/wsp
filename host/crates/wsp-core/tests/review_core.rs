// SPDX-License-Identifier: AGPL-3.0-only
//! Review probes for wsp-core (wsp-labs/wsp-map#2448). Each fails on 11db4a4e6.
mod common;
mod support;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use support::run_local;
use tokio::io::AsyncReadExt;
use wsp_core::daemon_channel::{open_daemon_channel, DaemonChannelOptions};
use wsp_core::reach::{connect_daemon, ReachOptions};

/// The first byte a dial sends to a listener on this computer, or `None` when nothing came inside 3 s.
async fn first_byte_of_a_dial(dial: impl FnOnce(u16)) -> Option<u8> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let first = tokio::task::spawn_local(async move {
        let (mut s, _) = listener.accept().await.ok()?;
        let mut b = [0u8; 1];
        s.read_exact(&mut b).await.ok().map(|_| b[0])
    });
    dial(port);
    tokio::time::timeout(Duration::from_secs(3), first).await.ok().and_then(|j| j.ok()).flatten()
}

// TS reach.ts:49-54 and :209: an https preview url is dialled as wss by node's ws, whose first byte is a TLS
// handshake record (22). Node printed 22 for the same url.
#[test]
fn a_reach_to_an_https_preview_url_opens_with_a_tls_handshake_as_the_ts_dial_does() {
    run_local(async {
        let held = Rc::new(RefCell::new(None));
        let keep = held.clone();
        let byte = first_byte_of_a_dial(move |port| {
            let reach = connect_daemon(ReachOptions {
                preview_url: Some(format!("https://127.0.0.1:{port}/?port=7681")),
                socket: None,
                token: Some("reach-token".into()),
                on_event: Rc::new(|_| {}),
                on_status: None,
                heartbeat_ms: None,
                seal: None,
                backoff_ms: Some(Rc::new(|_| 60_000)),
                clock: None,
                ids: None,
            });
            *keep.borrow_mut() = Some(reach);
        })
        .await;
        assert_eq!(byte, Some(22));
    });
}

// TS daemon-channel.ts:75: `new WebSocket(daemonWsUrl(o.url))`, the same TLS dial for an https url.
#[test]
fn a_daemon_channel_to_an_https_url_opens_with_a_tls_handshake_as_the_ts_dial_does() {
    run_local(async {
        let byte = first_byte_of_a_dial(|port| {
            tokio::task::spawn_local(open_daemon_channel(DaemonChannelOptions {
                url: format!("https://127.0.0.1:{port}/?port=7681"),
                token: "channel-token".into(),
                on_event: Rc::new(|_| {}),
                connect_timeout_ms: None,
                clock: None,
            }));
        })
        .await;
        assert_eq!(byte, Some(22));
    });
}

// Control for the harness above: the same reach over plain http sends its upgrade (`G` of GET, 71).
#[test]
fn control_a_reach_to_an_http_preview_url_sends_its_upgrade() {
    run_local(async {
        let held = Rc::new(RefCell::new(None));
        let keep = held.clone();
        let byte = first_byte_of_a_dial(move |port| {
            *keep.borrow_mut() = Some(connect_daemon(ReachOptions {
                preview_url: Some(format!("http://127.0.0.1:{port}/?port=7681")),
                socket: None,
                token: Some("reach-token".into()),
                on_event: Rc::new(|_| {}),
                on_status: None,
                heartbeat_ms: None,
                seal: None,
                backoff_ms: Some(Rc::new(|_| 60_000)),
                clock: None,
                ids: None,
            }));
        })
        .await;
        assert_eq!(byte, Some(71));
    });
}

// internal.ts:462-465 with transcripts.ts:407-430: the threads re-fold in the order the trim named them (a JS Set keeps
// insertion order: the cut thread, then the trimmed ones), so the index's words, and the index file written from them,
// list them in that order. Named t3 then t1, the TS answers [t2, t3, t1].
#[test]
fn reindex_threads_refolds_in_the_order_the_threads_were_named() {
    let chat = common::fixture("internal")["chat"]["events"].as_array().unwrap().clone();
    let mut index = wsp_core::types::internal::index_of(&common::wires(&serde_json::Value::Array(chat.clone())));
    let of = |t: &str| chat.iter().filter(|e| e["threadId"] == t).map(common::wire).collect::<Vec<_>>();
    let named = ["t3", "t1"];
    let threads: BTreeSet<String> = named.iter().map(|t| t.to_string()).collect();
    wsp_core::types::internal::reindex_threads(&mut index, &threads, &of);
    assert_eq!(index.words.keys().cloned().collect::<Vec<_>>(), vec!["t2", "t3", "t1"]);
}
