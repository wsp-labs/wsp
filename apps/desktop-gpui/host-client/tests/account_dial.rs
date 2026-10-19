// SPDX-License-Identifier: AGPL-3.0-only
//! The one dial at the account's host is given up after 8 s when it answers nothing, as `ACCOUNT_DIAL_MS` in
//! apps/desktop/src/host-lifecycle.ts bounds it: against a listener here that takes the socket and never speaks.

use std::time::{Duration, Instant};

use serde_json::json;
use tokio::net::TcpListener;
use wsp_host_client::{Env, HostClient};

/// A port that takes every socket and holds it without a word.
async fn silent_host() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((tcp, _)) = listener.accept().await {
            held.push(tcp);
        }
    });
    port
}

/// Catches: the account dial waiting the line's own window (5 s on loopback, 15 s past it) instead of the app's 8 s.
#[tokio::test]
async fn a_silent_account_host_is_given_up_after_eight_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    std::fs::create_dir_all(home.join("hosts")).unwrap();
    let port = silent_host().await;
    let record = json!({
        "url": format!("http://127.0.0.1:{port}"),
        "deviceId": "this-computer",
        "deviceToken": "a-device-token",
        "via": { "kind": "account", "hostId": "the-box" },
    });
    std::fs::write(home.join("hosts/box.json"), record.to_string()).unwrap();
    let env: Env = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let from = Instant::now();
    let dialled = HostClient::connect(&home.join("state.json"), &env).await;
    let waited = from.elapsed();
    let Err(refused) = dialled else { panic!("a host that never spoke was attached") };
    assert!(
        waited >= Duration::from_millis(7_500) && waited < Duration::from_millis(9_500),
        "the account dial was given up after {waited:?}, not about 8 s: {}",
        refused.message
    );
}
