// SPDX-License-Identifier: AGPL-3.0-only
//! The Changes pane's road to the host, apart from the daemon link: what it reads of the workspace and the thread (the
//! thread's folder, the project's, whether an agent works there now), the viewed marks, the pushes that change them,
//! and the workspace verbs it sends (viewed, discard, commit draft, commit, checkout).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    SessionsListRequest, WorkspacesCheckoutRequest, WorkspacesCommitDraftRequest, WorkspacesCommitRequest, WorkspacesDiscardRequest,
    WorkspacesListRequest, WorkspacesViewedRequest,
};
use wsp_host_client::{Event, Failure, HostClient, Op};

/// What the pane reads of the workspace and its thread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The thread's working folder, as its newest row says; none before its first start.
    pub followed: Option<String>,
    /// The project's folder as the workspace holds it: a copy's own folder where it has one.
    pub project: Option<String>,
    /// `running`, `napping` and the rest; none for a workspace the host does not list.
    pub phase: Option<String>,
    /// A turn runs in the workspace now.
    pub working: bool,
}

#[derive(Debug)]
pub enum Push {
    Facts(Facts),
    /// The workspace's viewed marks, path to the blob each was marked at.
    Marks(HashMap<String, String>),
}

async fn call<O: Op + DeserializeOwned>(host: &HostClient, params: Value) -> Result<O::Reply, Failure> {
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
    host.call(op).await
}

fn str_at(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// What the pane holds of the workspace to answer its facts: the project's folder and phase from the workspace list,
/// and the workspace's rows by session id, kept up to date from each pushed row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    pub project: Option<String>,
    pub phase: Option<String>,
    pub rows: HashMap<String, Value>,
}

impl Read {
    pub fn facts(&self, thread_id: Option<&str>) -> Facts {
        let working = self.rows.values().any(|r| r.get("status").and_then(Value::as_str) == Some("running"));
        let at = |r: &Value| r.get("startedAt").and_then(Value::as_f64).unwrap_or(0.);
        let followed = self
            .rows
            .values()
            .filter(|r| thread_id.is_some() && r.get("threadId").and_then(Value::as_str) == thread_id)
            .max_by(|a, b| at(a).total_cmp(&at(b)))
            .and_then(|r| str_at(r, "cwd"));
        Facts { followed, project: self.project.clone(), phase: self.phase.clone(), working }
    }

    /// A `session.row` push: its row put in place, or taken out where it is gone or of another workspace.
    pub fn apply_row(&mut self, frame: &Value, workspace_id: &str) {
        let Some(id) = frame.get("id").and_then(Value::as_str) else { return };
        match frame.get("row").filter(|r| r.get("workspaceId").and_then(Value::as_str) == Some(workspace_id)) {
            Some(row) => _ = self.rows.insert(id.to_owned(), row.clone()),
            None => _ = self.rows.remove(id),
        }
    }
}

async fn read_workspace(host: &HostClient, workspace_id: &str, read: &mut Read) -> Result<(), Failure> {
    let listed = call::<WorkspacesListRequest>(host, json!({})).await?;
    let workspaces = serde_json::to_value(&listed.workspaces).unwrap_or_default();
    let workspace = workspaces.as_array().into_iter().flatten().find(|w| w.get("id").and_then(Value::as_str) == Some(workspace_id));
    read.project = workspace.and_then(|w| str_at(w, "folder").or_else(|| w.get("project").and_then(|p| str_at(p, "path"))));
    read.phase = workspace.and_then(|w| str_at(w, "phase"));
    Ok(())
}

async fn read_rows(host: &HostClient, workspace_id: &str, read: &mut Read) -> Result<(), Failure> {
    let sessions = call::<SessionsListRequest>(host, json!({ "workspaceId": workspace_id })).await?;
    read.rows =
        sessions.sessions.iter().filter_map(|s| serde_json::to_value(s).ok()).filter_map(|r| Some((str_at(&r, "id")?, r))).collect();
    Ok(())
}

/// The workspace's viewed marks.
pub async fn marks(host: &HostClient, workspace_id: &str) -> Result<HashMap<String, String>, Failure> {
    Ok(call::<WorkspacesViewedRequest>(host, json!({ "workspaceId": workspace_id })).await?.viewed.into_iter().collect())
}

/// Marks `path` viewed at `blob`, or clears its mark with none; answers every mark the workspace holds now.
pub async fn mark(host: &HostClient, workspace_id: &str, path: &str, blob: Option<&str>) -> Result<HashMap<String, String>, Failure> {
    let marked = call::<WorkspacesViewedRequest>(host, json!({ "workspaceId": workspace_id, "path": path, "blob": blob })).await?;
    Ok(marked.viewed.into_iter().collect())
}

pub async fn discard(host: &HostClient, workspace_id: &str, path: &str) -> Result<(), Failure> {
    call::<WorkspacesDiscardRequest>(host, json!({ "workspaceId": workspace_id, "path": path })).await.map(|_| ())
}

/// The draft the workspace's agent writes for `paths`: its message, or none with the note saying why.
pub async fn commit_draft(
    host: &HostClient,
    workspace_id: &str,
    paths: &[String],
    thread_id: Option<&str>,
) -> Result<(Option<String>, Option<String>), Failure> {
    let mut params = json!({ "workspaceId": workspace_id, "paths": paths });
    if let Some(thread) = thread_id {
        params["threadId"] = json!(thread);
    }
    let draft = call::<WorkspacesCommitDraftRequest>(host, params).await?;
    Ok((draft.message, draft.note))
}

/// Commits `paths` with `message`; answers the commit's subject. The checkout is read again after, as the header's
/// counts follow it.
pub async fn commit(host: &HostClient, workspace_id: &str, message: &str, paths: &[String]) -> Result<String, Failure> {
    let made = call::<WorkspacesCommitRequest>(host, json!({ "workspaceId": workspace_id, "message": message, "paths": paths })).await?;
    _ = checkout(host, workspace_id).await;
    Ok(made.subject)
}

pub async fn checkout(host: &HostClient, workspace_id: &str) -> Result<(), Failure> {
    call::<WorkspacesCheckoutRequest>(host, json!({ "workspaceId": workspace_id })).await.map(|_| ())
}

fn kind_of(event: &Event) -> Option<(String, Value)> {
    let value = match event {
        Event::Host(e) => serde_json::to_value(e).ok()?,
        Event::Other(v) => v.clone(),
        _ => return None,
    };
    Some((value.get("type")?.as_str()?.to_owned(), value))
}

/// What a push asks of the pane's read: a row to put in place, the rows read again, or the workspace read again.
#[derive(Debug, PartialEq, Eq)]
enum Again {
    Row,
    Rows,
    Workspace,
}

fn again(kind: &str, frame: &Value, workspace_id: &str) -> Option<Again> {
    let of_mine = |v: &Value| v.get("workspaceId").and_then(Value::as_str) == Some(workspace_id);
    match kind {
        "session.row" => Some(Again::Row),
        "session.start" | "session.end" | "thread.rewound" if of_mine(frame) => Some(Again::Rows),
        "workspace.status" if frame.get("status").is_some_and(|s| s.get("id").and_then(Value::as_str) == Some(workspace_id)) => {
            Some(Again::Workspace)
        }
        k if k.starts_with("workspace.")
            && k != "workspace.viewed"
            && k != "workspace.sys"
            && k != "workspace.status"
            && of_mine(frame) =>
        {
            Some(Again::Workspace)
        }
        _ => None,
    }
}

/// Reads the facts and marks once, then sends each change the host pushes, until the receiver is gone; a socket that
/// closes is dialled again a second later and everything read again. A pushed row is put in place without a read.
pub async fn follow(
    host: Arc<HostClient>,
    workspace_id: String,
    thread_id: Option<String>,
    tx: mpsc::UnboundedSender<Push>,
) -> Result<(), Failure> {
    let gone = || Failure::new("gone");
    loop {
        let ended: Result<(), Failure> = async {
            let mut events = host.events().await?;
            let mut read = Read::default();
            // A refused read still lets the pane read git in the daemon's home, as the web's root falls back to it.
            _ = read_workspace(&host, &workspace_id, &mut read).await;
            _ = read_rows(&host, &workspace_id, &mut read).await;
            let mut sent = read.facts(thread_id.as_deref());
            tx.unbounded_send(Push::Facts(sent.clone())).map_err(|_| gone())?;
            tx.unbounded_send(Push::Marks(marks(&host, &workspace_id).await.unwrap_or_default())).map_err(|_| gone())?;
            while let Some(event) = events.next().await {
                let Ok(event) = event else { continue };
                let Some((kind, frame)) = kind_of(&event) else { continue };
                if kind == "workspace.viewed" && frame.get("workspaceId").and_then(Value::as_str) == Some(workspace_id.as_str()) {
                    let viewed = frame.get("viewed").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
                    tx.unbounded_send(Push::Marks(viewed)).map_err(|_| gone())?;
                    continue;
                }
                match again(&kind, &frame, &workspace_id) {
                    Some(Again::Row) => read.apply_row(&frame, &workspace_id),
                    Some(Again::Rows) => read_rows(&host, &workspace_id, &mut read).await?,
                    Some(Again::Workspace) => read_workspace(&host, &workspace_id, &mut read).await?,
                    None => continue,
                }
                let facts = read.facts(thread_id.as_deref());
                if facts != sent {
                    sent = facts;
                    tx.unbounded_send(Push::Facts(sent.clone())).map_err(|_| gone())?;
                }
            }
            Err(Failure::new("the host closed the socket"))
        }
        .await;
        if ended.is_ok() || tx.is_closed() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_workspaces_pushes_read_again_and_a_row_is_put_in_place() {
        assert_eq!(again("session.start", &json!({ "workspaceId": "w" }), "w"), Some(Again::Rows));
        assert_eq!(again("session.start", &json!({ "workspaceId": "x" }), "w"), None);
        assert_eq!(again("session.row", &json!({ "id": "s", "row": null }), "w"), Some(Again::Row));
        assert_eq!(again("workspace.status", &json!({ "status": { "id": "w" } }), "w"), Some(Again::Workspace));
        assert_eq!(again("workspace.viewed", &json!({ "workspaceId": "w" }), "w"), None);

        let mut read = Read::default();
        let row = |id: &str, ws: &str, status: &str, at: f64, cwd: &str| json!({ "id": id, "row": { "id": id, "workspaceId": ws, "threadId": "t", "status": status, "startedAt": at, "cwd": cwd } });
        read.apply_row(&row("s1", "w", "completed", 1., "/old"), "w");
        read.apply_row(&row("s2", "w", "running", 2., "/new"), "w");
        read.apply_row(&row("s3", "x", "running", 3., "/theirs"), "w");
        let facts = read.facts(Some("t"));
        assert_eq!((facts.followed.as_deref(), facts.working), (Some("/new"), true), "the newest row of the thread, and its turn running");
        read.apply_row(&json!({ "id": "s2", "row": null }), "w");
        let facts = read.facts(Some("t"));
        assert_eq!((facts.followed.as_deref(), facts.working), (Some("/old"), false), "a row gone is taken out");
    }
}
