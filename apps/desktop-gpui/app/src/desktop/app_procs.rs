// SPDX-License-Identifier: AGPL-3.0-only
//! The app's own processes, named for the host on this computer (`apps/desktop/src/app-procs.ts`), so Activity lists
//! this app's process as the app's. This app is one process, so the list is its own pid as the main one. It is sent
//! when a page starts watching and then every sample, and not at all while nobody watches. A host that predates the
//! offer refuses it, and nothing more is sent on that socket.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{App, Global};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use tokio::task::AbortHandle;
use wsp_host_client::ops::activity::Offered;
use wsp_host_client::protocol::{AppProcsOfferRequest, AppProcsSetRequest};
use wsp_host_client::{Event, HostClient};

use crate::screens::settings::model::req;

/// `DAEMON_SAMPLER_INTERVAL_MS`: a list each time the daemon samples.
const EVERY: Duration = Duration::from_millis(2000);
/// The wait before the host's socket, once it ended, is offered on again.
const AGAIN: Duration = Duration::from_secs(2);

struct Offering;

impl Global for Offering {}

/// Offers the app's processes to the host on this computer for as long as the app runs, once however many windows
/// connect; a host on the account is offered nothing.
pub fn offer_once(host: Arc<HostClient>, runtime: &Runtime, cx: &mut App) {
    if host.here() && !cx.has_global::<Offering>() {
        offer(host, runtime);
        cx.set_global(Offering);
    }
}

/// What this app names: its own process, the main one.
pub fn procs() -> Value {
    json!([{ "pid": std::process::id(), "kind": "main" }])
}

/// The offer on each socket the host client holds, one after another, until the handle is aborted.
pub fn offer(host: Arc<HostClient>, runtime: &Runtime) -> AbortHandle {
    let task = runtime.spawn(async move {
        loop {
            offer_on_socket(&host).await;
            tokio::time::sleep(AGAIN).await;
        }
    });
    task.abort_handle()
}

fn wanted_of(event: Event) -> Option<bool> {
    let frame = match event {
        Event::Other(frame) => frame,
        Event::Host(e) => serde_json::to_value(&*e).ok()?,
        _ => return None,
    };
    (frame.get("type").and_then(Value::as_str) == Some("app.procs.wanted"))
        .then(|| frame.get("wanted").and_then(Value::as_bool) == Some(true))
}

/// One socket's offer, until its events end.
async fn offer_on_socket(host: &HostClient) {
    let Ok(mut events) = host.events().await else { return };
    let offered: Result<Offered, _> = host.call(req::<AppProcsOfferRequest>(json!({}))).await;
    let refused = offered.is_err();
    let mut wanted = offered.is_ok_and(|o| o.wanted);
    let send = || async {
        _ = host.call(req::<AppProcsSetRequest>(json!({ "procs": procs() }))).await;
    };
    if wanted {
        send().await;
    }
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + EVERY, EVERY);
    loop {
        tokio::select! {
            next = events.next() => match next {
                None => return,
                Some(Ok(event)) if !refused => {
                    if let Some(now) = wanted_of(event) {
                        wanted = now;
                        if wanted {
                            send().await;
                            tick.reset();
                        }
                    }
                }
                _ => {}
            },
            _ = tick.tick(), if wanted => send().await,
        }
    }
}
