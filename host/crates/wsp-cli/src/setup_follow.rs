// SPDX-License-Identifier: AGPL-3.0-only
//! One add or one setup followed on the host's socket, and the tool's add that answers at the first wait, the end or
//! its ceiling. Port of `packages/host/src/setup-follow.ts` at 65f6b89c7; the spec is `spec/place_roads.md`.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::sync::Notify;
use wsp_wire::types::check::parse;
use wsp_wire::views::place::PlaceSetupLineState;
use wsp_wire::views::place::{
    AddLine, AddLineStep, PlaceSetupEvent, PlaceSetupLine, PlaceStageEvent, PlaceStageEventState, PlaceWait, PlaceWaitState, SetupEnd,
};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::place_word::provider_key_words;

use crate::verbs::client::{Frame, FrameOff, FrameSink, HostClient};

/// setup-follow.ts:16: the last end said, with what it came to.
#[derive(Clone, Debug, PartialEq)]
pub struct EndSaid {
    pub end: SetupEnd,
    pub said: Option<String>,
}

/// What has been heard of one add or setup so far (setup-follow.ts:11-24).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heard {
    pub lines: Vec<AddLine>,
    /// By row, in first-heard order, each as it last stood.
    pub waits: IndexMap<String, PlaceWait>,
    pub end: Option<EndSaid>,
    pub failed: Option<String>,
}

/// What `watch_setup` tells as it hears (setup-follow.ts:27).
pub struct SetupSay {
    pub line: Arc<dyn Fn(&AddLine) + Send + Sync>,
    pub wait: Arc<dyn Fn(&PlaceWait) + Send + Sync>,
}

/// setup-follow.ts:11-24: a stream followed. `heard` is the state so far.
pub struct SetupWatch {
    pub heard: Arc<Mutex<Heard>>,
    ids: Arc<Mutex<Vec<String>>>,
    off: Option<FrameOff>,
    wake: Arc<Notify>,
}

impl SetupWatch {
    /// setup-follow.ts:20: settles at the next line, wait or end heard after this call.
    pub async fn next(&self) {
        self.wake.notified().await
    }
    /// setup-follow.ts:22: follows a second add id as this one's.
    pub fn also(&self, add_id: &str) {
        let mut ids = self.ids.lock().unwrap();
        if !ids.iter().any(|id| id == add_id) {
            ids.push(add_id.to_string());
        }
    }
    /// setup-follow.ts:23.
    pub fn off(&mut self) {
        if let Some(off) = self.off.take() {
            off.off();
        }
    }
}

/// setup-follow.ts:27-68. `say` absent hears nothing.
pub fn watch_setup(client: &dyn HostClient, add_id: &str, say: Option<SetupSay>) -> SetupWatch {
    let heard: Arc<Mutex<Heard>> = Arc::default();
    let ids = Arc::new(Mutex::new(vec![add_id.to_string()]));
    let wake = Arc::new(Notify::new());
    let (h, i, w) = (heard.clone(), ids.clone(), wake.clone());
    let sink: FrameSink = Arc::new(move |frame: &Frame| {
        let value = Value::Object(frame.clone());
        if let Ok(stage) = parse::<PlaceStageEvent>(&value) {
            if is_followed(&i, &stage.add_id) {
                let line = AddLine {
                    step: AddLineStep::PlaceAddStep(stage.step),
                    state: stage_state(stage.state),
                    note: stage.note.clone(),
                    ms: None,
                    started_at: None,
                };
                {
                    let mut heard = h.lock().unwrap();
                    heard.lines.push(line.clone());
                    if stage.state == PlaceStageEventState::Failed {
                        heard.failed = stage.note.clone();
                    }
                }
                if let Some(say) = &say {
                    (say.line)(&line);
                }
                w.notify_waiters();
                return;
            }
        }
        let Ok(setup) = parse::<PlaceSetupEvent>(&value) else { return };
        if !is_followed(&i, &setup.add_id) {
            return;
        }
        if let Some(line) = &setup.line {
            let line = setup_line(line);
            h.lock().unwrap().lines.push(line.clone());
            if let Some(say) = &say {
                (say.line)(&line);
            }
        }
        if let Some(wait) = &setup.wait {
            h.lock().unwrap().waits.insert(wait.row.clone(), wait.clone());
            if let Some(say) = &say {
                (say.wait)(wait);
            }
        }
        if let Some(end) = setup.end {
            h.lock().unwrap().end = Some(EndSaid { end, said: setup.said.clone() });
        }
        w.notify_waiters();
    });
    let off = client.on_frame(sink);
    SetupWatch { heard, ids, off: Some(off), wake }
}

fn is_followed(ids: &Mutex<Vec<String>>, add_id: &str) -> bool {
    ids.lock().unwrap().iter().any(|id| id == add_id)
}

fn stage_state(state: PlaceStageEventState) -> PlaceSetupLineState {
    match state {
        PlaceStageEventState::Running => PlaceSetupLineState::Running,
        PlaceStageEventState::Done => PlaceSetupLineState::Done,
        PlaceStageEventState::Failed => PlaceSetupLineState::Failed,
    }
}

fn setup_line(line: &PlaceSetupLine) -> AddLine {
    AddLine {
        step: AddLineStep::PlaceSetupStep(line.step),
        state: line.state,
        note: line.note.clone(),
        ms: line.ms,
        started_at: line.started_at.clone(),
    }
}

/// setup-follow.ts:71.
pub fn open_waits(waiting: &[PlaceWait]) -> Vec<PlaceWait> {
    waiting.iter().filter(|w| w.state == PlaceWaitState::Waiting).cloned().collect()
}

/// setup-follow.ts:74.
pub fn add_tool_refusal(word: &str) -> String {
    format!("add takes a computer over ssh, by user@host or by an alias from the ssh config, and {word} is not one")
}
/// setup-follow.ts:75.
pub const ADD_TOOL_FIX: &str = "A project is recorded with projects_add; a provider's key and the join code another computer types are handed out at the host's own terminal, with wsp add.";

/// setup-follow.ts:79.
pub const ADD_TOOL_MS: u64 = 600_000;

/// setup-follow.ts:83-87. Serialized `computer, setup, waiting`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct AddAnswer {
    pub computer: PlaceView,
    pub setup: Vec<AddLine>,
    pub waiting: Vec<PlaceWait>,
}

/// setup-follow.ts:90.
pub fn is_computer_word(word: &str) -> bool {
    !word.contains(['/', ':']) && !word.starts_with(['~', '.']) && !word.ends_with(".git") && provider_key_words(word).is_none()
}

/// setup-follow.ts:96: the tool's ask.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AddAsk {
    pub address: String,
    pub recipe: Option<String>,
    pub later: Option<bool>,
    pub resume: Option<bool>,
    pub name: Option<String>,
    pub ssh_port: Option<i64>,
    pub key_path: Option<String>,
    pub host_key: Option<String>,
}

/// setup-follow.ts:96-128. `ceiling_ms` absent is `ADD_TOOL_MS`.
pub async fn add_computer(client: Arc<dyn HostClient>, ask: &AddAsk, ceiling_ms: Option<u64>) -> Result<AddAnswer, HostError> {
    if ask.resume != Some(true) && !is_computer_word(&ask.address) {
        return Err(usage_refusal(&add_tool_refusal(&ask.address), ADD_TOOL_FIX));
    }
    let wake = Arc::new(Notify::new());
    let listener = wake.clone();
    let off = client.on_frame(Arc::new(move |frame: &Frame| {
        if frame.get("type").and_then(Value::as_str) == Some("place.setup") {
            listener.notify_waiters();
        }
    }));
    let answer = add_session(client.as_ref(), ask, ceiling_ms, &wake).await;
    off.off();
    answer
}

async fn add_session(client: &dyn HostClient, ask: &AddAsk, ceiling_ms: Option<u64>, wake: &Notify) -> Result<AddAnswer, HostError> {
    client.events().await?;
    let mut recipe = Map::new();
    if let Some(r) = &ask.recipe {
        recipe.insert("recipe".into(), json!(r));
    }
    let reply = if ask.resume == Some(true) {
        let mut params = Map::new();
        params.insert("ref".into(), json!(ask.address));
        params.extend(recipe);
        client.request("places.setup", params).await?
    } else {
        let mut params = Map::new();
        params.insert("address".into(), json!(ask.address));
        if let Some(name) = &ask.name {
            params.insert("name".into(), json!(name));
        }
        if let Some(port) = ask.ssh_port {
            params.insert("sshPort".into(), json!(port));
        }
        if let Some(key) = &ask.key_path {
            params.insert("keyPath".into(), json!(key));
        }
        if let Some(host_key) = &ask.host_key {
            params.insert("hostKey".into(), json!(host_key));
        }
        params.extend(recipe);
        client.request("places.add", params).await?
    };
    let place = place_of(&reply)?;
    let until = Instant::now() + Duration::from_millis(ceiling_ms.unwrap_or(ADD_TOOL_MS));
    loop {
        let heard = wake.notified();
        tokio::pin!(heard);
        let listed = client.request("places.list", Map::new()).await?;
        let rows = listed.get("places").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut now = place.clone();
        for row in rows {
            let view: PlaceView = serde_json::from_value(row).map_err(|e| HostError::new(e.to_string()))?;
            if view.id == place.id {
                now = view;
                break;
            }
        }
        let waiting: Vec<PlaceWait> = now.setup.as_ref().map(|s| s.waiting.clone()).unwrap_or_default();
        let over = now.setup.as_ref().is_none_or(|s| s.state != wsp_wire::views::place::PlaceSetupState::Running);
        if over || (ask.later != Some(true) && !open_waits(&waiting).is_empty()) || Instant::now() >= until {
            let setup = now.setup.as_ref().map(|s| s.steps.iter().map(setup_line).collect()).unwrap_or_default();
            return Ok(AddAnswer { computer: now, setup, waiting });
        }
        let left = until.saturating_duration_since(Instant::now());
        tokio::select! {
            _ = &mut heard => {}
            _ = client.closed() => {}
            _ = tokio::time::sleep(left) => {}
        }
    }
}

fn place_of(reply: &Frame) -> Result<PlaceView, HostError> {
    let place = reply.get("place").cloned().unwrap_or(Value::Null);
    serde_json::from_value(place).map_err(|e| HostError::new(e.to_string()))
}
