// SPDX-License-Identifier: AGPL-3.0-only
//! The desktop app's road to the host, with no UI in it: one socket dialled as wsp dials it (wsp-mcp's own dial, with
//! the token beside the state file and the release check), every op the web app sends as a typed call, the events the
//! host pushes, the projects and threads the sidebar lists, and one thread's transcript followed live. The types the
//! host sends and takes are generated from record/protocol.json, which record.mjs writes off the protocol package's
//! zod schemas.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wsp_mcp::client::{Client, Frames};
use wsp_mcp::host::Host;

pub use wsp_mcp::failure::Failure;
pub use wsp_mcp::Env;

pub mod protocol {
    typify::import_types!(schema = "record/protocol.json");
}

pub mod ops;

use protocol::{
    AgentsSignInEvent, AppProcsEvent, DaemonChannelEvent, EventUnion, PlaceSysEvent, ProjectView, SessionEvent, SessionView,
    WorkspaceSysEvent,
};

const RECORD: &str = include_str!("../record/protocol.json");

/// HOST_ENV and the launch pair of LAUNCH_ENV in packages/protocol/src/env.ts.
const AIMS_A_LINE: [&str; 4] = ["WSP_HOST", "WSP_HOST_URL", "WSP_HOST_TOKEN", "WSP_HOST_KEY"];

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
}

impl HostClient {
    /// The host serving `state` here, else the account's one host, as the desktop app attaches. WSP_HOST and a turn's
    /// launch pair are not read: they aim a command line, and a turn's token sees its own thread's tree alone. Nothing
    /// is started: no host answering is a refusal.
    pub async fn connect(state: &Path, env: &Env) -> Result<HostClient, Failure> {
        let env: Env = env.iter().filter(|(name, _)| !AIMS_A_LINE.contains(&name.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        let args = wsp_mcp::Args { state: state.to_owned(), ..wsp_mcp::Args::default() };
        let host = Host::new(&args, &env, None);
        host.client().await?;
        Ok(HostClient { host })
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
            history,
            frames,
            _client: client,
        })
    }
}

pub struct Thread {
    pub thread_id: String,
    /// The transcript as it stood when the thread was opened, oldest first.
    pub history: Vec<SessionEvent>,
    /// The pos of the history's newest event; a pushed event at or under it is already in the history.
    through: u64,
    frames: Frames,
    /// The socket the frames ride, which closes when its last holder lets go.
    _client: Arc<Client>,
}

/// What the host pushed about one thread: a transcript event, typed, or anything else it says of the thread
/// (thread.head, session.queued) as it was sent.
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
            if frame.get("threadId").and_then(Value::as_str) != Some(self.thread_id.as_str()) {
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
    serde_json::to_value(event).ok()?.get("pos")?.as_u64()
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
    fn read(kind: &str, frame: Value) -> Result<Event, Failure> {
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
