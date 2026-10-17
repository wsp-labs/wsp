// SPDX-License-Identifier: AGPL-3.0-only
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;
use wsp_host_client::protocol::*;
use wsp_host_client::{Done, Event, Op};

use crate::common::{answered, ask, refused, shared};

/// What the desktop app sends to name its own processes; the web app sends neither, so the record holds no request.
#[derive(Serialize)]
struct Offer {}

impl Op for Offer {
    const NAME: &'static str = "appProcs.offer";
    type Reply = serde_json::Value;
}

#[derive(Serialize)]
struct Set {
    procs: serde_json::Value,
}

impl Op for Set {
    const NAME: &'static str = "appProcs.set";
    type Reply = Done;
}

#[tokio::test]
async fn activity() {
    let client = shared().host.client().await;

    let mut sleeper = Command::new("sleep").arg("60").stdin(Stdio::null()).spawn().unwrap();
    let started = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as f64;
    // Processes are ended through the daemon, and a throwaway host stages none.
    refused(
        "activity.end",
        client.call(ask::<ActivityEndRequest>(json!({ "pid": sleeper.id(), "startedAt": started }))).await,
        "the daemon for this computer did not start",
    );
    let still = sleeper.try_wait().unwrap();
    let _ = sleeper.kill();
    let _ = sleeper.wait();
    assert!(still.is_none(), "the refused end still ended the process: {still:?}");

    let mut events = client.events().await.unwrap();
    answered("appProcs.watch", client.call(ask::<AppProcsWatchRequest>(json!({}))).await);

    let app = shared().host.client().await;
    let offered = app.call(Offer {}).await.unwrap_or_else(|f| panic!("appProcs.offer: {}", f.message));
    assert_eq!(offered["wanted"], true, "{offered}");
    let pid = std::process::id();
    app.call(Set { procs: json!([{ "pid": pid, "kind": "main" }]) }).await.unwrap_or_else(|f| panic!("appProcs.set: {}", f.message));
    let pushed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.next().await {
                Some(Ok(Event::AppProcs(e))) => return e,
                Some(Ok(_)) => continue,
                Some(Err(f)) if f.message.starts_with("app.procs") => panic!("{}", f.message),
                Some(Err(_)) => continue,
                None => panic!("the socket closed before app.procs came"),
            }
        }
    })
    .await
    .expect("no app.procs pushed after appProcs.set");
    assert!(pushed.procs.0.iter().any(|p| p.pid.get() == pid as u64 && p.kind == AppProcListItemKind::Main), "{pushed:?}");

    answered("appProcs.unwatch", client.call(ask::<AppProcsUnwatchRequest>(json!({}))).await);
}
