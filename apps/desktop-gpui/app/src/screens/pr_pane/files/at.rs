// SPDX-License-Identifier: AGPL-3.0-only
//! What the Files pane, the finder and Open read about the thread's workspace off the host: its project folder, the
//! folder the thread's agent last worked in, whether it runs, and whether it is on this computer.

use std::time::Duration;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::{Context, Task};
use serde_json::{json, Value};
use wsp_host_client::{protocol, Event, Failure, HostClient, Op};

use crate::screens::pr_pane::changes::host::Read as Rows;
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

/// The thread's folder: the newest of its rows that names one, as today's store keeps the last folder a turn named. A
/// new turn's row names none until the runtime settles where it runs, which must not send the panes to the project.
pub fn thread_cwd(rows: &Rows, thread_id: &str) -> Option<String> {
    let at = |r: &Value| r.get("startedAt").and_then(Value::as_f64).unwrap_or(0.);
    rows.rows
        .values()
        .filter(|r| r.get("threadId").and_then(Value::as_str) == Some(thread_id))
        .filter_map(|r| Some((at(r), r.get("cwd")?.as_str().filter(|c| !c.is_empty())?)))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, cwd)| cwd.to_owned())
}

/// Reads the facts of `workspace_id`, with the thread's own folder when a thread is named, keeping the thread's rows
/// in `rows` for the pushes after.
async fn read_on(client: &HostClient, workspace_id: &str, thread_id: Option<&str>, rows: &mut Rows) -> Read {
    let get: protocol::WorkspacesGetRequest = request(json!({ "workspaceId": workspace_id })).map_err(|f| f.message)?;
    let got = client.call(get).await.map_err(|f| f.message)?;
    let view = serde_json::to_value(&got.workspace).unwrap_or_default();
    let mut facts = facts_of(&view);
    if let Some(thread) = thread_id {
        let list: protocol::SessionsListRequest = request(json!({ "workspaceId": workspace_id })).map_err(|f| f.message)?;
        if let Ok(listed) = client.call(list).await {
            let mine = listed.sessions.iter().filter(|r| r.thread_id.as_deref() == Some(thread));
            rows.rows = mine.filter_map(|r| Some((r.id.clone(), serde_json::to_value(r).ok()?))).collect();
            facts.cwd = thread_cwd(rows, thread);
        }
    }
    Ok(facts)
}

/// A pushed row of the thread put in place among `rows`, and the facts it leaves: the folder stays the newest row's,
/// so a late push of an earlier turn's row moves nothing.
pub fn row_pushed(rows: &mut Rows, facts: &Facts, frame: &Value, workspace_id: &str, thread_id: &str) -> Facts {
    rows.apply_row(frame, workspace_id);
    Facts { cwd: thread_cwd(rows, thread_id), ..facts.clone() }
}

/// What a push does to the facts.
#[derive(Debug, PartialEq, Eq)]
pub enum Moves {
    Nothing,
    /// The workspace itself changed, its phase among it: read it again.
    Workspace,
    /// A row of the thread moved or went, which can move the thread's folder.
    Row,
}

/// What a push does to the facts of `workspace_id` and its thread: the workspace changing reads it again, and a row
/// push of the thread is put in place, so no row asks for the workspace's whole list.
pub fn moves(frame: &Value, workspace_id: &str, thread_id: Option<&str>) -> Moves {
    let mine = |v: &Value| v.get("workspaceId").and_then(Value::as_str) == Some(workspace_id);
    match frame.get("type").and_then(Value::as_str).unwrap_or_default() {
        "session.row" if thread_id.is_some() => {
            let gone = frame.get("row").is_none_or(Value::is_null);
            let ours = frame.get("row").is_some_and(|r| mine(r) && r.get("threadId").and_then(Value::as_str) == thread_id);
            if gone || ours {
                Moves::Row
            } else {
                Moves::Nothing
            }
        }
        "workspace.status" if frame.pointer("/status/id").and_then(Value::as_str) == Some(workspace_id) => Moves::Workspace,
        kind if kind.starts_with("workspace.") && kind != "workspace.viewed" && kind != "workspace.status" && mine(frame) => {
            Moves::Workspace
        }
        _ => Moves::Nothing,
    }
}

/// Reads the facts, then again on each push that moves them, sending each change until the receiver goes; a socket
/// that closes is listened on again a second later.
fn watch(
    host: &Host,
    workspace_id: String,
    thread_id: Option<String>,
    tx: mpsc::UnboundedSender<Read>,
) -> impl std::future::Future<Output = ()> + 'static {
    let watching = host.run(move |client| async move {
        loop {
            if let Ok(mut events) = client.events().await {
                let mut rows = Rows::default();
                let mut facts = read_on(&client, &workspace_id, thread_id.as_deref(), &mut rows).await;
                if tx.unbounded_send(facts.clone()).is_err() {
                    return Ok(());
                }
                while let Some(event) = events.next().await {
                    let frame = match event {
                        Ok(Event::Host(e)) => serde_json::to_value(&e).unwrap_or_default(),
                        Ok(Event::Other(v)) => v,
                        _ => continue,
                    };
                    let next = match (moves(&frame, &workspace_id, thread_id.as_deref()), &facts, thread_id.as_deref()) {
                        (Moves::Workspace, _, _) => read_on(&client, &workspace_id, thread_id.as_deref(), &mut rows).await,
                        (Moves::Row, Ok(was), Some(thread)) => Ok(row_pushed(&mut rows, was, &frame, &workspace_id, thread)),
                        _ => continue,
                    };
                    if next == facts {
                        continue;
                    }
                    facts = next;
                    if tx.unbounded_send(facts.clone()).is_err() {
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

/// Follows the facts for the entity `cx` belongs to: `take` runs on the first read and on each change, until the
/// entity or the returned task goes.
pub fn follow<T: 'static>(
    host: &Host,
    workspace_id: String,
    thread_id: Option<String>,
    cx: &mut Context<T>,
    take: impl Fn(&mut T, Read, &mut Context<T>) + 'static,
) -> Task<()> {
    let (tx, mut rx) = mpsc::unbounded();
    let watching = watch(host, workspace_id, thread_id, tx);
    cx.spawn(async move |this, cx| {
        let hearing = async move {
            while let Some(read) = rx.next().await {
                if this.update(cx, |this, cx| take(this, read, cx)).is_err() {
                    return;
                }
            }
        };
        futures::future::select(Box::pin(watching), Box::pin(hearing)).await;
    })
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
