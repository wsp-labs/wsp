// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar's road to the host: one read of what it draws, the pushes that change it, and its writes, each through
//! `wsp-host-client` on the app's tokio runtime, answered back on the UI thread.

use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{
    ForwardsListRequest, InitGetRequest, PlacesListRequest, PreferencesGetRequest, ProjectsListRequest, ReleaseGetRequest,
    SessionsListRequest, StatusSubscribeRequest, WorkspacesListRequest,
};
use wsp_host_client::{Event, Failure, HostClient, Op};

/// What the host holds that the sidebar draws, as the JSON it sent, which the model reads by the protocol's names.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub sessions: Vec<Value>,
    pub workspaces: Vec<Value>,
    pub projects: Vec<Value>,
    pub places: Vec<Value>,
    pub preferences: Value,
    pub job: Option<Value>,
    /// Each workspace's live status (`status.subscribe`), which carries its checkout and pull request.
    pub statuses: Vec<Value>,
    pub release: Option<Value>,
    pub forwards: Vec<Value>,
}

/// What a push from the host changes.
#[derive(Debug)]
pub enum Change {
    /// The read again after the socket came back, or a push that names no one row.
    Whole(Box<Snapshot>),
    /// `session.row`: the row with that id, or its removal.
    Row {
        id: String,
        row: Option<Value>,
    },
    Preferences(Value),
    Places(Vec<Value>),
    Job(Option<Value>),
    Workspaces(Vec<Value>),
    Projects(Vec<Value>),
    /// `workspace.deleted`: the workspace went, and every row, status and port of it with it.
    WorkspaceDeleted(String),
    /// `workspace.status`: one workspace's status as it stands now.
    Status(Value),
    /// `workspace.creating`: a step of a workspace being made.
    Creating(Value),
    /// `workspace.created`: the workspace stands, so its creation row goes.
    Created(Value),
    Release(Value),
    Forward(Value),
    ForwardClosed {
        workspace_id: String,
        port: u64,
    },
    /// A start, a hold or an end that ties a send in flight to its thread, or ends it.
    Launched(Value),
    /// The road to the host went down; the sidebar says so until the next read lands.
    Lost(String),
}

/// A push that makes the sidebar read a part again rather than patch it.
enum Again {
    Sessions,
    Places,
    Job,
    Workspaces,
    Projects,
}

fn kind_of(event: &Event) -> Option<(String, Value)> {
    let value = match event {
        Event::Host(e) => serde_json::to_value(e).ok()?,
        Event::PlaceSys(e) => serde_json::to_value(e).ok()?,
        Event::Other(v) => v.clone(),
        _ => return None,
    };
    Some((value.get("type")?.as_str()?.to_owned(), value))
}

async fn call<O: Op + DeserializeOwned>(host: &HostClient, params: Value) -> Result<O::Reply, Failure> {
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
    host.call(op).await
}

fn values<T: serde::Serialize>(items: &[T]) -> Vec<Value> {
    items.iter().filter_map(|i| serde_json::to_value(i).ok()).collect()
}

/// Reads everything the sidebar draws; with `subscribe`, also asks for workspace statuses, which a socket asks once.
pub async fn read(host: &HostClient, subscribe: bool) -> Result<Snapshot, Failure> {
    let statuses = if subscribe {
        call::<StatusSubscribeRequest>(host, json!({})).await.map(|s| values(&s.statuses)).unwrap_or_default()
    } else {
        Vec::new()
    };
    let release = call::<ReleaseGetRequest>(host, json!({})).await.ok().and_then(|r| serde_json::to_value(&r.release).ok());
    let forwards = call::<ForwardsListRequest>(host, json!({})).await.map(|f| values(&f.forwards)).unwrap_or_default();
    let (sessions, workspaces, projects, places, preferences, setup) = futures::try_join!(
        call::<SessionsListRequest>(host, json!({})),
        call::<WorkspacesListRequest>(host, json!({})),
        call::<ProjectsListRequest>(host, json!({})),
        call::<PlacesListRequest>(host, json!({})),
        call::<PreferencesGetRequest>(host, json!({})),
        call::<InitGetRequest>(host, json!({})),
    )?;
    Ok(Snapshot {
        sessions: values(&sessions.sessions),
        workspaces: values(&workspaces.workspaces),
        projects: values(&projects.projects),
        places: values(&places.places),
        preferences: serde_json::to_value(&preferences.preferences).unwrap_or_default(),
        job: setup.setup.job.as_ref().and_then(|j| serde_json::to_value(j).ok()),
        statuses,
        release,
        forwards,
    })
}

async fn again(host: &HostClient, what: Again) -> Result<Change, Failure> {
    Ok(match what {
        Again::Sessions => Change::Whole(Box::new(Box::pin(read(host, false)).await?)),
        Again::Places => Change::Places(values(&call::<PlacesListRequest>(host, json!({})).await?.places)),
        Again::Job => {
            Change::Job(call::<InitGetRequest>(host, json!({})).await?.setup.job.as_ref().and_then(|j| serde_json::to_value(j).ok()))
        }
        Again::Workspaces => Change::Workspaces(values(&call::<WorkspacesListRequest>(host, json!({})).await?.workspaces)),
        Again::Projects => Change::Projects(values(&call::<ProjectsListRequest>(host, json!({})).await?.projects)),
    })
}

/// Reads everything once, then sends each change the host pushes, until the receiver is gone. A socket that closes is
/// dialled again a second later and everything read again, since `session.row` is not replayed.
pub fn follow(host: Arc<HostClient>, runtime: &Runtime, tx: mpsc::UnboundedSender<Change>) -> tokio::task::AbortHandle {
    runtime
        .spawn(async move {
            loop {
                let ended: Result<(), Failure> = async {
                    // Listening before the read, so nothing pushed between the two is lost.
                    // The read is boxed: built inline, its frame overflows a 2 MiB tokio worker's stack in a debug build.
                    let mut events = host.events().await?;
                    tx.unbounded_send(Change::Whole(Box::new(Box::pin(read(&host, true)).await?))).map_err(|_| Failure::new("gone"))?;
                    while let Some(event) = events.next().await {
                        // A frame of a newer protocol than the record is skipped: the next read brings what it said.
                        let Ok(event) = event else { continue };
                        let Some((kind, frame)) = kind_of(&event) else { continue };
                        let change = match kind.as_str() {
                            "session.row" => {
                                let id = frame.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                                Some(Ok(Change::Row { id, row: frame.get("row").filter(|r| !r.is_null()).cloned() }))
                            }
                            "preferences.changed" => frame.get("preferences").cloned().map(|p| Ok(Change::Preferences(p))),
                            "thread.rewound" => Some(again(&host, Again::Sessions).await),
                            "project.added" | "project.removed" => Some(again(&host, Again::Projects).await),
                            "init.job" => Some(again(&host, Again::Job).await),
                            "workspace.status" => frame.get("status").cloned().map(|s| Ok(Change::Status(s))),
                            "workspace.creating" => Some(Ok(Change::Creating(frame))),
                            "workspace.created" => {
                                tx.unbounded_send(Change::Created(frame)).map_err(|_| Failure::new("gone"))?;
                                Some(again(&host, Again::Workspaces).await)
                            }
                            "release.changed" => frame.get("release").cloned().map(|r| Ok(Change::Release(r))),
                            "forward.open" => frame.get("forward").cloned().map(|f| Ok(Change::Forward(f))),
                            "forward.close" => Some(Ok(Change::ForwardClosed {
                                workspace_id: frame.get("workspaceId").and_then(Value::as_str).unwrap_or_default().to_owned(),
                                port: frame.get("port").and_then(Value::as_u64).unwrap_or_default(),
                            })),
                            "session.start" | "session.held" | "session.end" => Some(Ok(Change::Launched(frame))),
                            "workspace.deleted" => {
                                frame.get("workspaceId").and_then(Value::as_str).map(|w| Ok(Change::WorkspaceDeleted(w.to_owned())))
                            }
                            k if k.starts_with("workspace.") && k != "workspace.sys" => Some(again(&host, Again::Workspaces).await),
                            // A place's own frames (its setup's steps, its readings) say nothing the foot reads.
                            "place.changed" | "place.removed" | "place.joined" | "place.present" | "place.absent" => {
                                Some(again(&host, Again::Places).await)
                            }
                            _ => None,
                        };
                        if let Some(change) = change {
                            tx.unbounded_send(change?).map_err(|_| Failure::new("gone"))?;
                        }
                    }
                    Err(Failure::new("the host closed the socket"))
                }
                .await;
                let Err(failure) = ended else { return };
                if tx.is_closed() || tx.unbounded_send(Change::Lost(failure.message)).is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .abort_handle()
}

/// One write, done or the host's refusal; what it changed comes back as pushes.
pub async fn write<O: Op + DeserializeOwned + Send>(host: Arc<HostClient>, params: Value) -> Result<(), Failure> {
    call::<O>(&host, params).await.map(drop)
}

/// One write and its reply as JSON, for a reply the sidebar reads: what a settle left standing.
pub async fn write_read<O: Op + DeserializeOwned + Send>(host: Arc<HostClient>, params: Value) -> Result<Value, Failure>
where
    O::Reply: serde::Serialize,
{
    let reply = call::<O>(&host, params).await?;
    Ok(serde_json::to_value(reply).unwrap_or_default())
}

/// One call and its reply, typed, for a reply the sidebar reads by its fields.
pub async fn ask<O: Op + DeserializeOwned + Send>(host: Arc<HostClient>, params: Value) -> Result<O::Reply, Failure> {
    call::<O>(&host, params).await
}
