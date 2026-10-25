// SPDX-License-Identifier: AGPL-3.0-only
//! One add or one setup followed on the host's socket, and the tool's add that answers at the first wait, the end or
//! its ceiling. Port of `packages/host/src/setup-follow.ts` at 65f6b89c7; the spec is `spec/place_roads.md`.
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use wsp_wire::views::place::{AddLine, PlaceWait, SetupEnd};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::HostError;

use crate::verbs::client::HostClient;

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
    off: Option<crate::verbs::client::FrameOff>,
    wake: Arc<tokio::sync::Notify>,
}

impl SetupWatch {
    /// setup-follow.ts:20: settles at the next line, wait or end heard after this call.
    pub async fn next(&self) {
        todo!()
    }
    /// setup-follow.ts:22: follows a second add id as this one's.
    pub fn also(&self, add_id: &str) {
        todo!()
    }
    /// setup-follow.ts:23.
    pub fn off(&mut self) {
        todo!()
    }
}

/// setup-follow.ts:27-68. `say` absent hears nothing.
pub fn watch_setup(client: &dyn HostClient, add_id: &str, say: Option<SetupSay>) -> SetupWatch {
    todo!()
}

/// setup-follow.ts:71.
pub fn open_waits(waiting: &[PlaceWait]) -> Vec<PlaceWait> {
    todo!()
}

/// setup-follow.ts:74.
pub fn add_tool_refusal(word: &str) -> String {
    todo!()
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
    todo!()
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
    todo!()
}
