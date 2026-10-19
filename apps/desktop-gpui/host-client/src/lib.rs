// SPDX-License-Identifier: AGPL-3.0-only
//! The desktop app's road to the host, with no UI in it: one socket dialled as wsp dials it (wsp-mcp's own dial, with
//! the token beside the state file and the release check), every op the web app sends as a typed call, the events the
//! host pushes, the projects and threads the sidebar lists, and one thread's transcript followed live. The types the
//! host sends and takes are generated from record/protocol.json, which record.mjs writes off the protocol package's
//! zod schemas.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wsp_mcp::client::{Client, Frames};
use wsp_mcp::host::Host;

pub use wsp_mcp::failure::Failure;
pub use wsp_mcp::{lock, release_order, Env};

/// The types build.rs generates from the record, each word list and tagged union with an `Unknown` arm that a value a
/// newer host sends and this release does not know reads as.
#[allow(non_camel_case_types, clippy::all)]
pub mod protocol {
    include!(concat!(env!("OUT_DIR"), "/protocol.rs"));
}

pub mod ops;
pub mod start;

use protocol::{
    AgentsSignInEvent, AppProcsEvent, DaemonChannelEvent, EventUnion, PlaceSysEvent, ProjectView, SessionEvent, SessionView,
    WorkspaceSysEvent,
};

const RECORD: &str = include_str!("../record/protocol.json");

/// HOST_ENV and the launch pair of LAUNCH_ENV in packages/protocol/src/env.ts.
const AIMS_A_LINE: [&str; 4] = ["WSP_HOST", "WSP_HOST_URL", "WSP_HOST_TOKEN", "WSP_HOST_KEY"];

/// `ACCOUNT_DIAL_MS` (`desktop:host-lifecycle.ts`): how long the one dial at the account's host waits. A person who
/// opened the app is watching it, so a host that has not answered is one the window opens without. The floor under it
/// is what a relayed road was measured to hold: an edge whose tunnel has just come up answers a first frame at 5.8 s.
const ACCOUNT_DIAL: std::time::Duration = std::time::Duration::from_millis(8_000);

/// One op the host serves: its request is the params record.mjs recorded for it, and its reply what the host answers
/// once the frame's id and ok are taken off.
pub trait Op: Serialize {
    const NAME: &'static str;
    type Reply: DeserializeOwned;
}

/// The reply of an op the host answers with nothing past its id and ok.
#[derive(Debug, Deserialize)]
pub struct Done {}

pub struct HostClient {
    /// Keeps one socket across calls and dials again once it closes, as the tool server's host does.
    host: Host,
    /// Every workspace's status on the socket in use, kept by its pushes.
    statuses: tokio::sync::Mutex<Option<Statuses>>,
}

struct Statuses {
    socket: Weak<Client>,
    rows: Arc<Mutex<HashMap<String, Value>>>,
}

impl HostClient {
    /// The host serving `state` here, else the account's one host, as the desktop app attaches. WSP_HOST and a turn's
    /// launch pair are not read: they aim a command line, and a turn's token sees its own thread's tree alone. Nothing
    /// is started: no host answering is a refusal.
    pub async fn connect(state: &Path, env: &Env) -> Result<HostClient, Failure> {
        HostClient::connect_holding(state, env, None).await
    }

    /// `connect`, presenting to the host here the token a pairing bought earlier in this process, where one did.
    pub async fn connect_holding(state: &Path, env: &Env, paired: Option<String>) -> Result<HostClient, Failure> {
        let host = attached(state, env);
        if let Some(token) = paired {
            host.hold_token(token);
        }
        host.client().await?;
        Ok(HostClient { host, statuses: tokio::sync::Mutex::new(None) })
    }

    /// Every workspace's status as this socket last heard it. `status.subscribe` is asked once a socket, as the web
    /// app's store asks it at bind and at reconnect: each ask holds a watcher on the host until the socket closes and
    /// has the provider asked about every machine, which keeps a cloud machine from napping. The socket's
    /// `workspace.status` pushes keep the rows after it, and a `workspace.deleted` takes its row away.
    pub async fn statuses(&self) -> Result<Vec<Value>, Failure> {
        let socket = self.host.client().await?;
        let mut held = self.statuses.lock().await;
        let same = |s: &&Statuses| s.socket.upgrade().is_some_and(|on| Arc::ptr_eq(&on, &socket));
        if let Some(rows) = held.as_ref().filter(same).map(|s| s.rows.clone()) {
            return Ok(rows.lock().unwrap().values().cloned().collect());
        }
        // Listening before the snapshot is asked, so no push falls between the two.
        let mut frames = socket.frames();
        socket.events().await?;
        let reply: Value = socket.request("status.subscribe", Map::new()).await?;
        let rows: HashMap<String, Value> = reply["statuses"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|status| Some((status["id"].as_str()?.to_owned(), status.clone())))
            .collect();
        let rows = Arc::new(Mutex::new(rows));
        let kept = rows.clone();
        tokio::spawn(async move {
            while let Some(text) = frames.next().await {
                let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue };
                match (frame["type"].as_str(), frame["status"]["id"].as_str(), frame["workspaceId"].as_str()) {
                    (Some("workspace.status"), Some(id), _) => _ = kept.lock().unwrap().insert(id.to_owned(), frame["status"].clone()),
                    (Some("workspace.deleted"), _, Some(id)) => _ = kept.lock().unwrap().remove(id),
                    _ => {}
                }
            }
        });
        *held = Some(Statuses { socket: Arc::downgrade(&socket), rows: rows.clone() });
        let shown = rows.lock().unwrap().values().cloned().collect();
        Ok(shown)
    }

    /// The same host as `connect` finds, paired by a code its `wsp host pair` printed: the code goes as the first frame
    /// of a socket that presents no token, which then closes; the client dials with the device token it bought.
    pub async fn redeem(state: &Path, env: &Env, code: &str, name: &str) -> Result<HostClient, Failure> {
        let host = attached(state, env);
        host.redeem(code, name).await?;
        Ok(HostClient { host, statuses: tokio::sync::Mutex::new(None) })
    }

    /// The token a pairing bought for the host here, which an account host keeps in its record instead.
    pub fn paired_token(&self) -> Option<String> {
        self.host.held_token()
    }

    /// The release the host named when this client last dialled it; none from a host before the answer carried one.
    pub fn host_release(&self) -> Option<String> {
        self.host.host_release()
    }

    /// This app's own release, the one a host's is read against ("this app is <app>, the host is <v>").
    pub fn app_release() -> String {
        wsp_mcp::own_release()
    }

    /// Whether this client dials the host serving its state file on this computer.
    pub fn here(&self) -> bool {
        self.host.aims_here()
    }

    /// The alias of the account's host this client dials, where it dials one rather than the host here.
    pub fn alias(&self) -> Option<String> {
        self.host.aimed_alias()
    }

    /// One op and its reply, typed both ways.
    pub async fn call<O: Op>(&self, op: O) -> Result<O::Reply, Failure> {
        let params = match serde_json::to_value(&op) {
            Ok(Value::Object(params)) => params,
            Ok(other) => return Err(Failure::new(format!("{} takes an object, not {other}", O::NAME))),
            Err(e) => return Err(Failure::new(format!("{}: {e}", O::NAME))),
        };
        let mut reply: Map<String, Value> = self.host.client().await?.request(O::NAME, params).await?;
        reply.remove("id");
        reply.remove("ok");
        serde_json::from_value(Value::Object(reply))
            .map_err(|e| Failure::new(format!("{} answered what its reply type does not read: {e}", O::NAME)))
    }

    /// One op's reply as the host sent it, for a reader that takes a few fields: nothing the record does not read
    /// fails it.
    pub async fn call_raw(&self, op: &str, params: Map<String, Value>) -> Result<Map<String, Value>, Failure> {
        let mut reply: Map<String, Value> = self.host.client().await?.request(op, params).await?;
        reply.remove("id");
        reply.remove("ok");
        Ok(reply)
    }

    /// Everything the host pushes on this socket from the moment this was asked, typed.
    pub async fn events(&self) -> Result<Events, Failure> {
        let client = self.host.client().await?;
        let frames = client.frames();
        client.events().await?;
        Ok(Events { frames, _client: client })
    }

    pub async fn projects(&self) -> Result<Vec<ProjectView>, Failure> {
        #[derive(Deserialize)]
        struct Listed {
            projects: Vec<ProjectView>,
        }
        Ok(self.host.client().await?.request::<Listed>("projects.list", Map::new()).await?.projects)
    }

    /// One row per thread, its newest turn's: a thread is the sessions that share a threadId, and a session from
    /// before threads is a thread of its own.
    pub async fn threads(&self) -> Result<Vec<SessionView>, Failure> {
        #[derive(Deserialize)]
        struct Listed {
            sessions: Vec<SessionView>,
        }
        let sessions = self.host.client().await?.request::<Listed>("sessions.list", Map::new()).await?.sessions;
        let mut rows: Vec<SessionView> = Vec::new();
        let mut row_of: HashMap<String, usize> = HashMap::new();
        for session in sessions {
            let thread = session.thread_id.clone().unwrap_or_else(|| session.id.clone());
            match row_of.get(&thread) {
                Some(&at) if session.started_at.unwrap_or(0.0) >= rows[at].started_at.unwrap_or(0.0) => rows[at] = session,
                Some(_) => {}
                None => {
                    row_of.insert(thread, rows.len());
                    rows.push(session);
                }
            }
        }
        Ok(rows)
    }

    /// One thread: every event of its transcript the host keeps, in one read, then each event of it the host pushes
    /// from the moment this was asked.
    pub async fn open(&self, workspace_id: &str, thread_id: &str) -> Result<Thread, Failure> {
        #[derive(Deserialize)]
        struct Whole {
            events: Vec<SessionEvent>,
        }
        let client = self.host.client().await?;
        // Listening before the transcript is read, so no event falls between the two; the overlap is dropped by pos.
        let frames = client.frames();
        client.events().await?;
        let mut params = Map::new();
        params.insert("workspaceId".to_owned(), Value::from(workspace_id));
        params.insert("threadId".to_owned(), Value::from(thread_id));
        params.insert("whole".to_owned(), Value::from(true));
        let history = client.request::<Whole>("sessions.history", params).await?.events;
        Ok(Thread {
            thread_id: thread_id.to_owned(),
            through: history.iter().rev().find_map(pos).unwrap_or(0),
            total: history.len() as u64,
            trimmed: false,
            history,
            frames,
            _client: client,
        })
    }

    /// One thread from its newest page: the newest `HISTORY_PAGE_EVENTS` events the host keeps of it, then each event of
    /// it the host pushes from the moment this was asked. Older pages come through `page`.
    pub async fn open_page(&self, workspace_id: &str, thread_id: &str) -> Result<Thread, Unopened> {
        let client = self.host.client().await.map_err(Unopened::Lost)?;
        // Listening before the page is read, so no event falls between the two; the overlap is dropped by pos.
        let frames = client.frames();
        client.events().await.map_err(Unopened::Lost)?;
        let page = match read_page(&client, workspace_id, thread_id, None).await {
            Ok(page) => page,
            Err(failure) if client.is_closed() => return Err(Unopened::Lost(failure)),
            Err(failure) => return Err(Unopened::Refused(failure)),
        };
        Ok(Thread {
            thread_id: thread_id.to_owned(),
            through: page.pos,
            history: page.events,
            total: page.total,
            trimmed: page.trimmed == Some(true),
            frames,
            _client: client,
        })
    }

    /// The page of one thread's events just older than the event at `before`, oldest first.
    pub async fn page(&self, workspace_id: &str, thread_id: &str, before: u64) -> Result<Page, Failure> {
        read_page(&*self.host.client().await?, workspace_id, thread_id, Some(before)).await
    }
}

/// How many events a page of a thread's history holds: HISTORY_PAGE_EVENTS in packages/protocol.
pub const HISTORY_PAGE_EVENTS: u64 = 200;

/// One page of a thread's events out of `sessions.history`: the newest under its `before`, oldest first. `pos` is the
/// newest position the workspace's transcript had issued when it was read, `total` how many events of the thread the
/// host keeps, and `trimmed` that the host's caps dropped older ones nothing can read again.
pub type Page = protocol::HistoryPage;

async fn read_page(client: &Client, workspace_id: &str, thread_id: &str, before: Option<u64>) -> Result<Page, Failure> {
    let mut params = Map::new();
    params.insert("workspaceId".to_owned(), Value::from(workspace_id));
    params.insert("threadId".to_owned(), Value::from(thread_id));
    params.insert("limit".to_owned(), Value::from(HISTORY_PAGE_EVENTS));
    if let Some(before) = before {
        params.insert("before".to_owned(), Value::from(before));
    }
    client.request::<Page>("sessions.history", params).await
}

/// Why a thread did not open: its socket closed, or the host answered the read with a refusal.
#[derive(Debug)]
pub enum Unopened {
    Lost(Failure),
    Refused(Failure),
}

/// The tool server's host for `state`, with the names that aim a command line taken out of the environment, attaching
/// to a host of a newer release than this app's as the desktop app does and refusing an older one, and refusing a
/// lock in the desktop app's words.
fn attached(state: &Path, env: &Env) -> Host {
    let env: Env = env.iter().filter(|(name, _)| !AIMS_A_LINE.contains(&name.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    let args = wsp_mcp::Args { state: state.to_owned(), ..wsp_mcp::Args::default() };
    Host::new(&args, &env, None).with_release_rule(wsp_mcp::ReleaseRule::OwnOrNewer).with_account_window(ACCOUNT_DIAL)
}

pub struct Thread {
    pub thread_id: String,
    /// The transcript as it stood when the thread was opened, oldest first: all of it from `open`, its newest page from
    /// `open_page`.
    pub history: Vec<SessionEvent>,
    /// How many events of the thread the host keeps, and whether its caps dropped older ones.
    pub total: u64,
    pub trimmed: bool,
    /// The pos of the history's newest event; a pushed event at or under it is already in the history.
    through: u64,
    frames: Frames,
    /// The socket the frames ride, which closes when its last holder lets go.
    _client: Arc<Client>,
}

/// What the host pushed about one thread: a transcript event, typed, or anything else it says of the thread
/// (thread.head, session.queued, its rows and its children's) as it was sent.
#[derive(Debug)]
pub enum Live {
    Event(Box<SessionEvent>),
    Other(Value),
}

impl Thread {
    /// The thread's next event as it arrives; none once the socket is gone.
    pub async fn next(&mut self) -> Option<Result<Live, Failure>> {
        loop {
            let text = self.frames.next().await?;
            let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue };
            let ours = frame.get("threadId").and_then(Value::as_str) == Some(self.thread_id.as_str());
            // A child thread's row moves what the thread shows of it: its state, its waiting question.
            let child = frame.get("type").and_then(Value::as_str) == Some("session.row")
                && frame.pointer("/row/parentThreadId").and_then(Value::as_str) == Some(self.thread_id.as_str());
            if !ours && !child {
                continue;
            }
            if frame.get("pos").and_then(Value::as_u64).is_some_and(|pos| pos <= self.through) {
                continue;
            }
            let kind = frame.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
            if !transcript_types().contains(&kind) {
                return Some(Ok(Live::Other(frame)));
            }
            return Some(
                serde_json::from_value(frame).map(|event| Live::Event(Box::new(event))).map_err(|e| Failure::new(format!("{kind}: {e}"))),
            );
        }
    }
}

/// Where an event sits in its thread's transcript, where it says.
pub fn pos(event: &SessionEvent) -> Option<u64> {
    use SessionEvent as E;
    let pos = match event {
        E::SessionStart { pos, .. }
        | E::SessionDelta { pos, .. }
        | E::SessionDone { pos, .. }
        | E::SessionEnd { pos, .. }
        | E::SessionSteer { pos, .. }
        | E::SessionNotify { pos, .. }
        | E::SessionPermission { pos, .. }
        | E::SessionPermissionClosed { pos, .. }
        | E::SessionCheckpoint { pos, .. }
        | E::SessionChanges { pos, .. }
        | E::SessionPlan { pos, .. }
        | E::SessionCompacted { pos, .. }
        | E::SessionRetry { pos, .. }
        | E::SessionContext { pos, .. }
        | E::SessionRun { pos, .. }
        | E::SessionMoved { pos, .. }
        | E::SessionBehind { pos, .. }
        | E::SessionNotice { pos, .. }
        | E::SessionEarlier { pos, .. }
        | E::SessionCapped { pos, .. }
        | E::SessionUncapped { pos, .. }
        | E::SessionSubagent { pos, .. }
        | E::SessionSlate { pos, .. }
        | E::ThreadWaited { pos, .. } => pos,
        E::Unknown => &None,
    };
    pos.map(std::num::NonZeroU64::get)
}

pub struct Events {
    frames: Frames,
    _client: Arc<Client>,
}

/// One frame the host pushed, by the schema the web app reads it with.
#[derive(Debug)]
pub enum Event {
    Host(Box<EventUnion>),
    SignIn(Box<AgentsSignInEvent>),
    Daemon(Box<DaemonChannelEvent>),
    Sys(Box<WorkspaceSysEvent>),
    PlaceSys(Box<PlaceSysEvent>),
    AppProcs(Box<AppProcsEvent>),
    /// A frame of a type no recorded schema names, or a daemon channel's that does not read, as it was sent.
    Other(Value),
}

impl Event {
    /// A pushed frame by the schema its type names. One of a recorded type that does not read as that type is a
    /// failure, since the record is then behind the host, except a daemon channel's: today's client routes those by
    /// their channel unread (`protocol/client.ts`).
    pub fn read(kind: &str, frame: Value) -> Result<Event, Failure> {
        let read = |e: serde_json::Error| Failure::new(format!("{kind}: {e}"));
        match kind {
            "workspace.sys" => serde_json::from_value(frame).map(|e| Event::Sys(Box::new(e))).map_err(read),
            "place.sys" => serde_json::from_value(frame).map(|e| Event::PlaceSys(Box::new(e))).map_err(read),
            "app.procs" => serde_json::from_value(frame).map(|e| Event::AppProcs(Box::new(e))).map_err(read),
            "agents.signIn" => serde_json::from_value(frame).map(|e| Event::SignIn(Box::new(e))).map_err(read),
            _ if kind.starts_with("daemon.") => {
                Ok(serde_json::from_value(frame.clone()).map_or(Event::Other(frame), |e| Event::Daemon(Box::new(e))))
            }
            _ if host_event_types().contains(kind) => serde_json::from_value(frame).map(|e| Event::Host(Box::new(e))).map_err(read),
            _ => Ok(Event::Other(frame)),
        }
    }
}

impl Events {
    /// The next frame as it arrives, read by [`Event::read`]; none once the socket is gone.
    pub async fn next(&mut self) -> Option<Result<Event, Failure>> {
        loop {
            let text = self.frames.next().await?;
            let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue };
            let Some(kind) = frame.get("type").and_then(Value::as_str).map(str::to_owned) else { continue };
            return Some(Event::read(&kind, frame));
        }
    }
}

/// The type of every event a transcript holds, read off the record the types were generated from.
fn transcript_types() -> &'static HashSet<String> {
    static TYPES: OnceLock<HashSet<String>> = OnceLock::new();
    TYPES.get_or_init(|| types_of(|unions| unions.transcript))
}

fn host_event_types() -> &'static HashSet<String> {
    static TYPES: OnceLock<HashSet<String>> = OnceLock::new();
    TYPES.get_or_init(|| types_of(|unions| unions.events))
}

/// The record's two unions, read field by field: read as a `Value`, the record is a 4 MB tree whose freed pages stay resident.
#[derive(Deserialize)]
struct Unions {
    #[serde(rename = "EventUnion")]
    events: Union,
    #[serde(rename = "SessionEvent")]
    transcript: Union,
}

#[derive(Deserialize)]
struct Union {
    #[serde(rename = "anyOf")]
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    properties: Option<VariantProperties>,
}

#[derive(Deserialize)]
struct VariantProperties {
    #[serde(rename = "type")]
    kind: Option<VariantType>,
}

#[derive(Deserialize)]
struct VariantType {
    #[serde(rename = "enum", default)]
    names: Vec<String>,
}

fn types_of(union: impl FnOnce(Unions) -> Union) -> HashSet<String> {
    #[derive(Deserialize)]
    struct Record {
        definitions: Unions,
    }
    let record: Record = serde_json::from_str(RECORD).expect("record/protocol.json names both unions");
    union(record.definitions).variants.into_iter().filter_map(|v| v.properties?.kind?.names.into_iter().next()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_channels_frame_that_does_not_read_comes_as_it_was_sent() {
        let closed = serde_json::json!({ "type": "daemon.closed", "channel": "c1" });
        assert!(matches!(Event::read("daemon.closed", closed.clone()), Ok(Event::Other(frame)) if frame == closed));
        let whole = serde_json::json!({ "type": "daemon.closed", "channel": "c1", "code": 1000, "reason": "" });
        assert!(matches!(Event::read("daemon.closed", whole), Ok(Event::Daemon(_))));
    }

    #[test]
    fn every_transcript_type_is_read_off_the_record() {
        let types = transcript_types();
        for kind in ["session.start", "session.delta", "session.done", "session.end"] {
            assert!(types.contains(kind), "{kind} is not in the record");
        }
        assert!(!types.contains("thread.head"));
        assert!(host_event_types().contains("thread.head"));
    }

    #[test]
    fn each_union_lists_what_the_whole_record_lists() {
        let record: Value = serde_json::from_str(RECORD).unwrap();
        for (union, types) in [("EventUnion", host_event_types()), ("SessionEvent", transcript_types())] {
            let variants = record["definitions"][union]["anyOf"].as_array().unwrap();
            let listed: HashSet<String> =
                variants.iter().map(|v| v["properties"]["type"]["enum"][0].as_str().unwrap().to_owned()).collect();
            assert_eq!(&listed, types, "{union}");
        }
    }

    /// A frame for the daemon goes with every key the page put in it: the host passes it through untouched.
    #[test]
    fn a_daemon_frame_keeps_its_params() {
        let asked = serde_json::json!({ "channel": "c", "frame": { "op": "pty.attach", "ptyId": "p1" } });
        let request: protocol::DaemonSendRequest = serde_json::from_value(asked.clone()).unwrap();
        assert_eq!(serde_json::to_value(&request).unwrap(), asked);
    }

    /// Two events of the union that each hold a `step`, one a word and one an object, read as their own types.
    #[test]
    fn a_field_two_events_shape_apart_reads_in_each() {
        let place = r#"{"type":"place.stage","addId":"a","step":"connect","state":"running","seq":1}"#;
        let golden = r#"{"type":"golden.stage","name":"default","stage":"creating","step":{"label":"l","command":"c"},"seq":2}"#;
        for frame in [place, golden] {
            serde_json::from_str::<EventUnion>(frame).unwrap_or_else(|e| panic!("{frame}: {e}"));
        }
    }

    /// The parity of the calls with the record: each op record.mjs found the web app sending has its call here, under
    /// the request type the record names for it, and no call is left for an op the record dropped.
    #[test]
    fn every_op_the_web_app_sends_has_a_call() {
        let record: Value = serde_json::from_str(RECORD).unwrap();
        let recorded: Vec<(String, String)> =
            record["ops"].as_object().unwrap().iter().map(|(op, request)| (op.clone(), request.as_str().unwrap().to_owned())).collect();
        let called: Vec<(String, String)> = ops::all().map(|(op, request)| (op.to_owned(), request.to_owned())).collect();
        let lacking: Vec<_> = recorded.iter().filter(|op| !called.contains(op)).collect();
        let stale: Vec<_> = called.iter().filter(|op| !recorded.contains(op)).collect();
        assert!(
            lacking.is_empty() && stale.is_empty(),
            "ops the web app sends with no call here: {lacking:?}; calls for ops it no longer sends: {stale:?}"
        );
    }
}
