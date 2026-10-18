// SPDX-License-Identifier: AGPL-3.0-only
//! The pane's road to the host: the workspace's status as it stands and each `workspace.status` push after it, and
//! every op the pane and the git button send, each answered as the JSON the host sent.

use std::future::Future;

use futures::channel::mpsc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_host_client::protocol::{PreferencesGetRequest, StatusSubscribeRequest, WorkspacesCheckoutRequest};
use wsp_host_client::{Event, Failure, Op};

use crate::screens::Host;

/// One op and its reply as JSON, the request read from `params` by the protocol's names.
pub fn ask<O>(host: &Host, params: Value) -> impl Future<Output = Result<Value, Failure>> + 'static
where
    O: Op + DeserializeOwned + Send + 'static,
    O::Reply: Serialize + Send,
{
    host.run(move |client| async move {
        let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
        let reply = client.call(op).await?;
        serde_json::to_value(reply).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))
    })
}

/// The JSON of a push, whatever union it rode in.
fn frame_of(event: &Event) -> Option<Value> {
    match event {
        Event::Host(e) => serde_json::to_value(e).ok(),
        Event::Other(v) => Some(v.clone()),
        _ => None,
    }
}

/// Sends the workspace's status as it stands, then each one the host pushes for it, until the receiver goes. It asks
/// the host to read the branch once, which is what reads the pull request on view.
pub fn follow(host: &Host, workspace_id: String, tx: mpsc::UnboundedSender<Value>) -> impl Future<Output = Result<(), Failure>> + 'static {
    host.run(move |client| async move {
        let mut events = client.events().await?;
        let snapshot: StatusSubscribeRequest = serde_json::from_value(json!({})).map_err(|e| Failure::new(e.to_string()))?;
        for status in client.call(snapshot).await?.statuses {
            let status = serde_json::to_value(status).unwrap_or_default();
            if status["id"] == workspace_id.as_str() {
                tx.unbounded_send(status).map_err(|_| Failure::new("gone"))?;
            }
        }
        let checkout: WorkspacesCheckoutRequest =
            serde_json::from_value(json!({ "workspaceId": workspace_id })).map_err(|e| Failure::new(e.to_string()))?;
        _ = client.call(checkout).await;
        while let Some(event) = events.next().await {
            let Ok(event) = event else { continue };
            let Some(frame) = frame_of(&event) else { continue };
            if frame["type"] == "workspace.status" && frame["status"]["id"] == workspace_id.as_str() {
                tx.unbounded_send(frame["status"].clone()).map_err(|_| Failure::new("gone"))?;
            }
        }
        Ok(())
    })
}

/// The agent every send to the agent reaches: the harness of the workspace's first thread, else the agent a new
/// thread would start on (`usePrAgent`).
pub fn agent_of(host: &Host, workspace_id: String) -> impl Future<Output = Result<String, Failure>> + 'static {
    host.run(move |client| async move {
        let first = client
            .threads()
            .await?
            .into_iter()
            .filter(|t| t.workspace_id == workspace_id && t.thread_id.is_some())
            .min_by(|a, b| a.started_at.unwrap_or(f64::MAX).total_cmp(&b.started_at.unwrap_or(f64::MAX)))
            .and_then(|t| serde_json::to_value(&t.harness).ok())
            .and_then(|h| h.as_str().map(str::to_owned));
        if let Some(first) = first {
            return Ok(first);
        }
        let ask: PreferencesGetRequest = serde_json::from_value(json!({})).map_err(|e| Failure::new(e.to_string()))?;
        let preferences = serde_json::to_value(client.call(ask).await?.preferences).unwrap_or_default();
        Ok(preferences["defaultAgent"].as_str().unwrap_or("claude").to_owned())
    })
}
