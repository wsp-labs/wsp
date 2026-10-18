// SPDX-License-Identifier: AGPL-3.0-only
//! What the Files pane, the finder and Open read about the thread's workspace off the host: its project folder, the
//! folder the thread's agent last worked in, whether it runs, and whether it is on this computer.

use std::time::Duration;

use futures::channel::mpsc;
use serde_json::{json, Value};
use wsp_host_client::{protocol, Event, Failure, HostClient, Op};

use crate::screens::Host;

/// The workspace's facts, as `useWorkspace` and the thread's rows hold them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub name: String,
    /// `projectFolderOf`: the copy's own folder where it has one, else the project's.
    pub project: Option<String>,
    /// The folder the thread's last turn ran in, as its row reports it.
    pub cwd: Option<String>,
    pub running: bool,
    /// `isLocalWorkspace`: a copy on this computer.
    pub here: bool,
    /// The machine's memory, where the host reports one.
    pub mem_mb: Option<u64>,
}

/// The facts, or the host's sentence for why it would not give them.
pub type Read = Result<Facts, String>;

pub(crate) fn request<O: Op + serde::de::DeserializeOwned>(params: Value) -> Result<O, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))
}

/// Reads the facts of `workspace_id`, with the thread's own folder when a thread is named.
pub async fn read(host: Host, workspace_id: String, thread_id: Option<String>) -> Read {
    host.run(move |client| async move { Ok(read_on(&client, &workspace_id, thread_id.as_deref()).await) })
        .await
        .unwrap_or_else(|f| Err(f.message))
}

async fn read_on(client: &HostClient, workspace_id: &str, thread_id: Option<&str>) -> Read {
    let get: protocol::WorkspacesGetRequest = request(json!({ "workspaceId": workspace_id })).map_err(|f| f.message)?;
    let got = client.call(get).await.map_err(|f| f.message)?;
    let view = serde_json::to_value(&got.workspace).unwrap_or_default();
    let mut facts = facts_of(&view);
    if let Some(thread) = thread_id {
        let list: protocol::SessionsListRequest = request(json!({ "workspaceId": workspace_id })).map_err(|f| f.message)?;
        if let Ok(listed) = client.call(list).await {
            let mut rows: Vec<_> = listed.sessions.into_iter().filter(|r| r.thread_id.as_deref() == Some(thread)).collect();
            rows.sort_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)));
            facts.cwd = rows.iter().rev().find_map(|r| r.cwd.clone()).filter(|c| !c.is_empty());
        }
    }
    Ok(facts)
}

/// Whether a push can move these facts: a row of the workspace, a turn starting or ending in it, or the workspace
/// itself changing phase.
pub fn moves_facts(frame: &Value, workspace_id: &str) -> bool {
    let mine = |v: &Value| v.get("workspaceId").and_then(Value::as_str) == Some(workspace_id);
    match frame.get("type").and_then(Value::as_str).unwrap_or_default() {
        "session.row" => frame.get("row").is_none_or(|r| r.is_null() || mine(r)),
        "session.start" | "session.end" => mine(frame),
        "workspace.status" => frame.pointer("/status/id").and_then(Value::as_str) == Some(workspace_id),
        kind => kind.starts_with("workspace.") && kind != "workspace.viewed" && mine(frame),
    }
}

/// Reads the facts, then again on each push that can move them, sending each read until the receiver goes; a socket
/// that closes is listened on again a second later.
pub fn watch(
    host: &Host,
    workspace_id: String,
    thread_id: Option<String>,
    tx: mpsc::UnboundedSender<Read>,
) -> impl std::future::Future<Output = ()> + 'static {
    let watching = host.run(move |client| async move {
        loop {
            if let Ok(mut events) = client.events().await {
                if tx.unbounded_send(read_on(&client, &workspace_id, thread_id.as_deref()).await).is_err() {
                    return Ok(());
                }
                while let Some(event) = events.next().await {
                    let frame = match event {
                        Ok(Event::Host(e)) => serde_json::to_value(&e).unwrap_or_default(),
                        Ok(Event::Other(v)) => v,
                        _ => continue,
                    };
                    if moves_facts(&frame, &workspace_id)
                        && tx.unbounded_send(read_on(&client, &workspace_id, thread_id.as_deref()).await).is_err()
                    {
                        return Ok(());
                    }
                }
            }
            if tx.is_closed() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    async move {
        _ = watching.await;
    }
}

/// The facts a workspace view carries.
pub fn facts_of(view: &Value) -> Facts {
    let text = |key: &str| view.get(key).and_then(Value::as_str).map(str::to_owned);
    let project = text("folder").or_else(|| view.pointer("/project/path").and_then(Value::as_str).map(str::to_owned));
    Facts {
        name: text("name").unwrap_or_default(),
        project,
        cwd: None,
        running: text("phase").as_deref() == Some("running"),
        here: text("kind").as_deref() == Some("local"),
        mem_mb: view.pointer("/size/memMb").and_then(Value::as_u64),
    }
}
