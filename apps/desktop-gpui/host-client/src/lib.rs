// SPDX-License-Identifier: AGPL-3.0-only
//! The desktop app's road to the host, with no UI in it: one socket dialled as wsp dials it (wsp-mcp's own dial, with
//! the token beside the state file and the release check), the projects and threads the sidebar lists, and one
//! thread's transcript followed live. The types the host sends are generated from record/protocol.json, which
//! record.mjs writes off the protocol package's zod schemas.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;
use serde_json::{Map, Value};
use wsp_mcp::client::{Client, Frames};
use wsp_mcp::host::Host;

pub use wsp_mcp::failure::Failure;
pub use wsp_mcp::Env;

pub mod protocol {
    typify::import_types!(schema = "record/protocol.json");
}

use protocol::{HistoryPage, ProjectView, SessionEvent, SessionView};

const RECORD: &str = include_str!("../record/protocol.json");

/// HOST_ENV and the launch pair of LAUNCH_ENV in packages/protocol/src/env.ts.
const AIMS_A_LINE: [&str; 4] = ["WSP_HOST", "WSP_HOST_URL", "WSP_HOST_TOKEN", "WSP_HOST_KEY"];

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

    /// One thread: the newest page of its transcript, as the app opens a thread, then each event of it the host pushes
    /// from the moment this was asked.
    pub async fn open(&self, workspace_id: &str, thread_id: &str) -> Result<Thread, Failure> {
        let client = self.host.client().await?;
        // Listening before the transcript is read, so no event falls between the two; the overlap is dropped by pos.
        let frames = client.frames();
        client.events().await?;
        let mut params = Map::new();
        params.insert("workspaceId".to_owned(), Value::from(workspace_id));
        params.insert("threadId".to_owned(), Value::from(thread_id));
        let page: HistoryPage = client.request("sessions.history", params).await?;
        Ok(Thread {
            workspace_id: workspace_id.to_owned(),
            thread_id: thread_id.to_owned(),
            history: page.events,
            through: page.pos,
            frames,
            client,
        })
    }
}

pub struct Thread {
    pub workspace_id: String,
    pub thread_id: String,
    /// The newest page of the transcript as it stood when the thread was opened, oldest first.
    pub history: Vec<SessionEvent>,
    /// The transcript's pos the history reaches; a pushed event at or under it is already in the history.
    through: u64,
    frames: Frames,
    /// The socket the frames ride, which closes when its last holder lets go.
    client: Arc<Client>,
}

/// The most events one page back asks for, HISTORY_PAGE_MAX in packages/protocol; the host also caps a page by bytes.
const PAGE_BACK: u64 = 1000;

/// What the host pushed about one thread: a transcript event, typed, or anything else it says of the thread
/// (thread.head, session.queued) as it was sent.
#[derive(Debug)]
pub enum Live {
    Event(Box<SessionEvent>),
    Other(Value),
}

impl Thread {
    /// The page of the transcript just before `before`, a pos, oldest first: how the app reads further back than the
    /// page it opened with. Empty once the transcript's start is reached.
    pub async fn earlier(&self, before: u64) -> Result<Vec<SessionEvent>, Failure> {
        let mut params = Map::new();
        params.insert("workspaceId".to_owned(), Value::from(self.workspace_id.as_str()));
        params.insert("threadId".to_owned(), Value::from(self.thread_id.as_str()));
        params.insert("before".to_owned(), Value::from(before));
        params.insert("limit".to_owned(), Value::from(PAGE_BACK));
        Ok(self.client.request::<HistoryPage>("sessions.history", params).await?.events)
    }

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

/// The type of every event a transcript holds, read off the record the types were generated from.
fn transcript_types() -> &'static HashSet<String> {
    static TYPES: OnceLock<HashSet<String>> = OnceLock::new();
    TYPES.get_or_init(|| {
        let record: Value = serde_json::from_str(RECORD).expect("record/protocol.json is JSON");
        let variants = record["definitions"]["SessionEvent"]["anyOf"].as_array().cloned().unwrap_or_default();
        variants.iter().filter_map(|v| v["properties"]["type"]["enum"][0].as_str().map(str::to_owned)).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_transcript_type_is_read_off_the_record() {
        let types = transcript_types();
        for kind in ["session.start", "session.delta", "session.done", "session.end"] {
            assert!(types.contains(kind), "{kind} is not in the record");
        }
        assert!(!types.contains("thread.head"));
    }
}
