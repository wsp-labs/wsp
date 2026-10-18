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
    /// `job.needs-you`: the need the build job now carries.
    JobNeed(Value),
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
#[derive(Debug)]
enum Again {
    Sessions,
    Places,
    Workspaces,
    Projects,
}

/// `frameEndsStep`: a setup frame that ends the run, lands a row, or carries a line no longer running.
fn ends_step(frame: &Value) -> bool {
    let has = |key: &str| frame.get(key).is_some_and(|v| !v.is_null());
    has("end")
        || has("landed")
        || frame.get("line").filter(|l| !l.is_null()).is_some_and(|l| l.get("state").and_then(Value::as_str) != Some("running"))
}

pub(crate) fn kind_of(event: &Event) -> Option<(String, Value)> {
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

pub(crate) fn values<T: serde::Serialize>(items: &[T]) -> Vec<Value> {
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
        Again::Sessions => Change::Whole(Box::new(read(host, false).await?)),
        Again::Places => Change::Places(values(&call::<PlacesListRequest>(host, json!({})).await?.places)),
        Again::Workspaces => Change::Workspaces(values(&call::<WorkspacesListRequest>(host, json!({})).await?.workspaces)),
        Again::Projects => Change::Projects(values(&call::<ProjectsListRequest>(host, json!({})).await?.projects)),
    })
}

/// What one push asks of the sidebar: a change it carries, a part read again, or both in that order.
#[derive(Debug)]
enum Step {
    Take(Change),
    Read(Again),
    TakeThenRead(Change, Again),
}

/// The sidebar's rule for a push of `kind`, none for a push it does not follow.
fn step(kind: &str, frame: Value) -> Option<Step> {
    let take = |change: Option<Change>| change.map(Step::Take);
    match kind {
        "session.row" => {
            let id = frame.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            take(Some(Change::Row { id, row: frame.get("row").filter(|r| !r.is_null()).cloned() }))
        }
        "preferences.changed" => take(frame.get("preferences").cloned().map(Change::Preferences)),
        "thread.rewound" => Some(Step::Read(Again::Sessions)),
        "project.added" | "project.removed" => Some(Step::Read(Again::Projects)),
        // `useStore`'s init.job: the frame carries the job whole, so it is taken as it stands.
        "init.job" => take(Some(Change::Job(frame.get("job").filter(|j| !j.is_null()).cloned()))),
        "job.needs-you" => take(frame.get("needsYou").filter(|n| !n.is_null()).cloned().map(Change::JobNeed)),
        "workspace.status" => take(frame.get("status").cloned().map(Change::Status)),
        "workspace.creating" => take(Some(Change::Creating(frame))),
        "workspace.created" => Some(Step::TakeThenRead(Change::Created(frame), Again::Workspaces)),
        "release.changed" => take(frame.get("release").cloned().map(Change::Release)),
        "forward.open" => take(frame.get("forward").cloned().map(Change::Forward)),
        "forward.close" => take(Some(Change::ForwardClosed {
            workspace_id: frame.get("workspaceId").and_then(Value::as_str).unwrap_or_default().to_owned(),
            port: frame.get("port").and_then(Value::as_u64).unwrap_or_default(),
        })),
        "session.start" | "session.held" | "session.end" => take(Some(Change::Launched(frame))),
        "workspace.deleted" => take(frame.get("workspaceId").and_then(Value::as_str).map(|w| Change::WorkspaceDeleted(w.to_owned()))),
        k if k.starts_with("workspace.") && k != "workspace.sys" => Some(Step::Read(Again::Workspaces)),
        "place.changed" | "place.removed" | "place.joined" | "place.present" | "place.absent" => Some(Step::Read(Again::Places)),
        // A step's rows and the end's outcome ride the record, not the frame: read once per step.
        "place.setup" if ends_step(&frame) => Some(Step::Read(Again::Places)),
        _ => None,
    }
}

/// Reads everything once, then sends each change the host pushes, until the receiver is gone. A socket that closes is
/// dialled again a second later and everything read again, since `session.row` is not replayed.
pub fn follow(host: Arc<HostClient>, runtime: &Runtime, tx: mpsc::UnboundedSender<Change>) -> tokio::task::AbortHandle {
    runtime
        .spawn(async move {
            loop {
                let ended: Result<(), Failure> = async {
                    // Listening before the read, so nothing pushed between the two is lost.
                    let mut events = host.events().await?;
                    tx.unbounded_send(Change::Whole(Box::new(read(&host, true).await?))).map_err(|_| Failure::new("gone"))?;
                    while let Some(event) = events.next().await {
                        // A frame of a newer protocol than the record is skipped: the next read brings what it said.
                        let Ok(event) = event else { continue };
                        let Some((kind, frame)) = kind_of(&event) else { continue };
                        match step(&kind, frame) {
                            Some(Step::Take(change)) => tx.unbounded_send(change).map_err(|_| Failure::new("gone"))?,
                            Some(Step::Read(what)) => tx.unbounded_send(again(&host, what).await?).map_err(|_| Failure::new("gone"))?,
                            Some(Step::TakeThenRead(change, what)) => {
                                tx.unbounded_send(change).map_err(|_| Failure::new("gone"))?;
                                tx.unbounded_send(again(&host, what).await?).map_err(|_| Failure::new("gone"))?;
                            }
                            None => {}
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ends_step, step, Again, Change, Step};

    #[test]
    fn a_setup_frame_that_ends_a_step_reads_the_computers_again() {
        let ended = step("place.setup", json!({ "placeId": "p", "addId": "s", "end": "ready" }));
        assert!(matches!(ended, Some(Step::Read(Again::Places))), "the sidebar reads the list again: {ended:?}");
        let landed = step("place.setup", json!({ "placeId": "p", "landed": { "row": "gh" } }));
        assert!(matches!(landed, Some(Step::Read(Again::Places))), "a landed row is read again: {landed:?}");
        assert!(step("place.setup", json!({ "placeId": "p", "line": { "state": "running" } })).is_none(), "a running line reads nothing");
    }

    #[test]
    fn a_pushed_job_is_taken_from_its_frame() {
        let pushed = step("init.job", json!({ "type": "init.job", "job": { "id": "j", "phase": "building" } }));
        let Some(Step::Take(Change::Job(Some(job)))) = pushed else { panic!("the frame's job is taken: {pushed:?}") };
        assert_eq!(job["id"], "j");
        let need = step("job.needs-you", json!({ "type": "job.needs-you", "jobId": "j", "needsYou": { "what": "Sign in" } }));
        let Some(Step::Take(Change::JobNeed(need))) = need else { panic!("the need is taken on the job's own road: {need:?}") };
        assert_eq!(need["what"], "Sign in");
    }

    #[test]
    fn a_setup_frame_ends_a_step_as_the_web_reads_it() {
        assert!(ends_step(&json!({ "placeId": "p", "end": "done" })));
        assert!(ends_step(&json!({ "placeId": "p", "landed": { "row": "gh" } })));
        assert!(ends_step(&json!({ "placeId": "p", "line": { "state": "done" } })));
        assert!(!ends_step(&json!({ "placeId": "p", "line": { "state": "running" } })));
        assert!(!ends_step(&json!({ "placeId": "p", "stage": "copy", "end": null })));
    }
}
