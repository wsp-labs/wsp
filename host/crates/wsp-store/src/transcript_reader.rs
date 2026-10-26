// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's events out of a workspace's transcript, and the one rule for an event's JSON and size. Port of
//! packages/runtime/src/transcript-reader.ts at 65f6b89c7 less `numbered`, which only numbers events an older build
//! wrote with no positions (Proposal 3 drops them); the spec is spec/transcript_reader.md.
use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::preferences::HEAD_RESULT_CHARS;
use wsp_wire::views::session_events::DeltaKind;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;
use wsp_words::tool_result::kept_patch;

/// What a read of one thread asks for (transcript-reader.ts:8-14): the events under `before` (every one when
/// `None`), newest first, at most `limit` of them (`usize::MAX` for the TS `Infinity`) and no more than `bytes` of
/// UTF-8 JSON as `shape` leaves each, the comma after each counted. `strict` holds even the newest event to the bytes.
#[derive(Clone, Copy)]
pub struct ThreadRead {
    pub before: Option<i64>,
    pub limit: usize,
    pub bytes: u64,
    pub strict: bool,
    pub shape: Option<fn(&SessionEvent) -> SessionEvent>,
}

/// The events read, oldest first; `pos` the newest position the workspace's transcript has issued, `total` how many
/// events of the thread it holds, `trimmed` whether the caps dropped older events of it (transcript-reader.ts:18-24).
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadEvents {
    pub events: Vec<SessionEvent>,
    pub pos: i64,
    pub total: u64,
    pub trimmed: bool,
}

/// What `read_thread` answers: `ThreadEvents` less `pos` and `trimmed`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadPick {
    pub events: Vec<SessionEvent>,
    pub total: u64,
}

/// One thread's events out of a workspace's transcript (transcript-reader.ts:28-30), what a head and a history page
/// read; the threads area implements it.
#[async_trait::async_trait(?Send)]
pub trait TranscriptReader {
    async fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError>;
}

/// The read over a transcript held as one list in the order its positions were issued (transcript-reader.ts:33-40).
pub fn read_thread(events: &[SessionEvent], thread_id: &str, o: &ThreadRead) -> ThreadPick {
    let of_thread: Vec<&SessionEvent> = events.iter().filter(|e| thread_of(e).as_deref() == Some(thread_id)).collect();
    let total = of_thread.len() as u64;
    ThreadPick { events: pick_newest(of_thread.into_iter().rev().cloned(), o), total }
}

/// What a read takes of one thread's events given newest first, answered oldest first (transcript-reader.ts:44-56).
pub fn pick_newest(newest: impl IntoIterator<Item = SessionEvent>, o: &ThreadRead) -> Vec<SessionEvent> {
    let mut picked: Vec<SessionEvent> = Vec::new();
    let mut bytes: u64 = 0;
    for e in newest {
        if o.before.is_some_and(|before| pos_of(&e) >= before) {
            continue;
        }
        let shaped = match o.shape {
            Some(shape) => shape(&e),
            None => e,
        };
        let size = json_stringify(&value_of(&shaped)).len() as u64 + 1;
        if picked.len() >= o.limit || ((o.strict || !picked.is_empty()) && bytes + size > o.bytes) {
            break;
        }
        picked.push(shaped);
        bytes += size;
    }
    picked.reverse();
    picked
}

/// An event's JSON as a transcript writes it, and the size its byte cap counts (transcript-reader.ts:60-63).
#[derive(Clone, Debug, PartialEq)]
pub struct EventJson {
    pub json: String,
    /// `json.length`: UTF-16 code units, not UTF-8 bytes (rule 2.11).
    pub bytes: u64,
}

/// `JSON.stringify(e)` through `wsp_wire::text::json_stringify`, and its length in UTF-16 units.
pub fn event_json(e: &SessionEvent) -> EventJson {
    let json = json_stringify(&value_of(e));
    let bytes = json.encode_utf16().count() as u64;
    EventJson { json, bytes }
}

/// transcript-reader.ts:65.
pub fn event_bytes(e: &SessionEvent) -> u64 {
    event_json(e).bytes
}

/// An event as a head carries it (transcript-reader.ts:77-81): a `tool_result` delta's text past `HEAD_RESULT_CHARS`
/// UTF-16 units cut there with `cut` its whole length, its patch cut by `kept_patch` to as many characters; every
/// other event as it is.
pub fn head_shape(e: &SessionEvent) -> SessionEvent {
    let SessionEvent::SessionDeltaEvent(delta) = e else {
        return e.clone();
    };
    if !matches!(delta.kind, DeltaKind::ToolResult) {
        return e.clone();
    }
    let mut shaped = delta.clone();
    let whole = shaped.text.encode_utf16().count() as u64;
    if whole > HEAD_RESULT_CHARS {
        shaped.text = cut_units(&shaped.text, HEAD_RESULT_CHARS);
        shaped.cut = Some(JsInt::from(whole as i64));
    }
    if let Some(patch) = shaped.patch.take() {
        let kept = kept_patch(&patch, HEAD_RESULT_CHARS as usize);
        shaped.patch = Some(kept.patch);
        if kept.patch_cut {
            shaped.patch_cut = Some(serde_json::from_value(Value::Bool(true)).expect("true reads as the patchCut flag"));
        }
    }
    SessionEvent::SessionDeltaEvent(shaped)
}

pub(crate) fn value_of(e: &SessionEvent) -> Value {
    serde_json::to_value(e).expect("a session event writes as JSON")
}

pub(crate) fn thread_of(e: &SessionEvent) -> Option<String> {
    value_of(e).get("threadId")?.as_str().map(str::to_owned)
}

pub(crate) fn pos_of(e: &SessionEvent) -> i64 {
    value_of(e).get("pos").and_then(Value::as_i64).unwrap_or(0)
}

/// The first `units` UTF-16 units of `text`, one unit fewer when the cut would leave the first half of a surrogate pair.
/// Only called on a text longer than `units`, so a high surrogate at the cut always has its low half after it.
fn cut_units(text: &str, units: u64) -> String {
    let mut wide: Vec<u16> = text.encode_utf16().take(units as usize).collect();
    if matches!(wide.last(), Some(0xD800..=0xDBFF)) {
        wide.pop();
    }
    String::from_utf16_lossy(&wide)
}
