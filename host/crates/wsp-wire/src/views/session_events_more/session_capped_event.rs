// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/session-events.ts:579 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCappedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub place_id: String,
    pub place: String,
    pub running: JsInt,
    pub at_once: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<SessionCappedEventWhy>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prompt: Option<String>,
}
impl SessionCappedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.capped";
}
impl Check for SessionCappedEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::opt_key::<String>(obj, "copiedFrom", &mut out);
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "place", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "running", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "atOnce", &mut out);
        crate::types::keys::opt_key::<SessionCappedEventWhy>(obj, "why", &mut out);
        crate::types::keys::opt_key::<String>(obj, "requestId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "prompt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "placeId" => <String as Check>::describe(rest),
                "place" => <String as Check>::describe(rest),
                "running" => <i64 as Check>::describe(rest),
                "atOnce" => <i64 as Check>::describe(rest),
                "why" => <Option<SessionCappedEventWhy> as Check>::describe(rest),
                "requestId" => <Option<String> as Check>::describe(rest),
                "prompt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionCappedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionUncappedEventWhy {
    #[serde(rename = "freed")]
    Freed,
    #[serde(rename = "raised")]
    Raised,
    #[serde(rename = "lent")]
    Lent,
}
impl Check for SessionUncappedEventWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["freed", "raised", "lent"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:592 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUncappedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub why: SessionUncappedEventWhy,
    pub at_once: JsInt,
}
impl SessionUncappedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.uncapped";
}
impl Check for SessionUncappedEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::opt_key::<String>(obj, "copiedFrom", &mut out);
        crate::types::keys::req_key::<SessionUncappedEventWhy>(obj, "why", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "atOnce", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "why" => <SessionUncappedEventWhy as Check>::describe(rest),
                "atOnce" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionUncappedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:602 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "exited")]
    Exited,
    #[serde(rename = "moved")]
    Moved,
    #[serde(rename = "lost")]
    Lost,
}
impl Check for RunState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "exited", "moved", "lost"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:609 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRunEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub run_id: String,
    pub block: String,
    pub command: String,
    pub state: RunState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pty_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub exit_code: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub signal: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub output: Option<String>,
}
impl SessionRunEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.run";
}
impl Check for SessionRunEvent {
    /// Rules: pos: > 0.
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::opt_key::<String>(obj, "copiedFrom", &mut out);
        crate::types::keys::req_key::<String>(obj, "runId", &mut out);
        crate::types::keys::req_key::<String>(obj, "block", &mut out);
        crate::types::keys::req_key::<String>(obj, "command", &mut out);
        crate::types::keys::req_key::<RunState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "ptyId", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "exitCode", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "signal", &mut out);
        crate::types::keys::opt_key::<String>(obj, "output", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "runId" => <String as Check>::describe(rest),
                "block" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                "state" => <RunState as Check>::describe(rest),
                "ptyId" => <Option<String> as Check>::describe(rest),
                "exitCode" => <Option<i64> as Check>::describe(rest),
                "signal" => <Option<i64> as Check>::describe(rest),
                "output" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionRunEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:625 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub run_id: String,
    pub block: String,
    pub command: String,
    pub state: RunState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pty_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub exit_code: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub signal: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub output: Option<String>,
    pub thread_id: String,
    pub turn_id: String,
}
impl Check for RunStep {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "runId", &mut out);
        crate::types::keys::req_key::<String>(obj, "block", &mut out);
        crate::types::keys::req_key::<String>(obj, "command", &mut out);
        crate::types::keys::req_key::<RunState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "ptyId", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "exitCode", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "signal", &mut out);
        crate::types::keys::opt_key::<String>(obj, "output", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "turnId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "runId" => <String as Check>::describe(rest),
                "block" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                "state" => <RunState as Check>::describe(rest),
                "ptyId" => <Option<String> as Check>::describe(rest),
                "exitCode" => <Option<i64> as Check>::describe(rest),
                "signal" => <Option<i64> as Check>::describe(rest),
                "output" => <Option<String> as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "turnId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:635 at a3b870a6e.
/// zod's discriminatedUnion on `type`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEvent {
    #[serde(rename = "session.start")]
    SessionStartEvent(SessionStartEvent),
    #[serde(rename = "session.delta")]
    SessionDeltaEvent(SessionDeltaEvent),
    #[serde(rename = "session.done")]
    SessionDoneEvent(SessionDoneEvent),
    #[serde(rename = "session.end")]
    SessionEndEvent(SessionEndEvent),
    #[serde(rename = "session.steer")]
    SessionSteerEvent(SessionSteerEvent),
    #[serde(rename = "session.notify")]
    SessionNotifyEvent(SessionNotifyEvent),
    #[serde(rename = "session.permission")]
    SessionPermissionEvent(SessionPermissionEvent),
    #[serde(rename = "session.permission.closed")]
    SessionPermissionClosedEvent(SessionPermissionClosedEvent),
    #[serde(rename = "session.checkpoint")]
    SessionCheckpointEvent(SessionCheckpointEvent),
    #[serde(rename = "session.changes")]
    SessionChangesEvent(SessionChangesEvent),
    #[serde(rename = "session.plan")]
    SessionPlanEvent(SessionPlanEvent),
    #[serde(rename = "session.compacted")]
    SessionCompactedEvent(SessionCompactedEvent),
    #[serde(rename = "session.retry")]
    SessionRetryEvent(SessionRetryEvent),
    #[serde(rename = "session.context")]
    SessionContextEvent(SessionContextEvent),
    #[serde(rename = "session.run")]
    SessionRunEvent(SessionRunEvent),
    #[serde(rename = "session.moved")]
    SessionMovedEvent(SessionMovedEvent),
    #[serde(rename = "session.behind")]
    SessionBehindEvent(SessionBehindEvent),
    #[serde(rename = "session.notice")]
    SessionNoticeEvent(SessionNoticeEvent),
    #[serde(rename = "session.earlier")]
    SessionEarlierEvent(SessionEarlierEvent),
    #[serde(rename = "session.capped")]
    SessionCappedEvent(SessionCappedEvent),
    #[serde(rename = "session.uncapped")]
    SessionUncappedEvent(SessionUncappedEvent),
    #[serde(rename = "session.subagent")]
    SessionSubagentEvent(SessionSubagentEvent),
    #[serde(rename = "session.slate")]
    SessionSlateEvent(SessionSlateEvent),
    #[serde(rename = "thread.waited")]
    ThreadWaitedEvent(ThreadWaitedEvent),
}
impl Check for SessionEvent {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            SessionEvent::SessionStartEvent(v) => v.check(),
            SessionEvent::SessionDeltaEvent(v) => v.check(),
            SessionEvent::SessionDoneEvent(v) => v.check(),
            SessionEvent::SessionEndEvent(v) => v.check(),
            SessionEvent::SessionSteerEvent(v) => v.check(),
            SessionEvent::SessionNotifyEvent(v) => v.check(),
            SessionEvent::SessionPermissionEvent(v) => v.check(),
            SessionEvent::SessionPermissionClosedEvent(v) => v.check(),
            SessionEvent::SessionCheckpointEvent(v) => v.check(),
            SessionEvent::SessionChangesEvent(v) => v.check(),
            SessionEvent::SessionPlanEvent(v) => v.check(),
            SessionEvent::SessionCompactedEvent(v) => v.check(),
            SessionEvent::SessionRetryEvent(v) => v.check(),
            SessionEvent::SessionContextEvent(v) => v.check(),
            SessionEvent::SessionRunEvent(v) => v.check(),
            SessionEvent::SessionMovedEvent(v) => v.check(),
            SessionEvent::SessionBehindEvent(v) => v.check(),
            SessionEvent::SessionNoticeEvent(v) => v.check(),
            SessionEvent::SessionEarlierEvent(v) => v.check(),
            SessionEvent::SessionCappedEvent(v) => v.check(),
            SessionEvent::SessionUncappedEvent(v) => v.check(),
            SessionEvent::SessionSubagentEvent(v) => v.check(),
            SessionEvent::SessionSlateEvent(v) => v.check(),
            SessionEvent::ThreadWaitedEvent(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else {
            return Some(vec![crate::types::check::missing::<Self>("type")]);
        };
        match tag.as_str() {
            Some("session.start") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionStartEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.delta") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionDeltaEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.done") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionDoneEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.end") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionEndEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.steer") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionSteerEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.notify") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionNotifyEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.permission") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionPermissionEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.permission.closed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionPermissionClosedEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.checkpoint") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionCheckpointEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.changes") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionChangesEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.plan") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionPlanEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.compacted") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionCompactedEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.retry") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionRetryEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.context") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionContextEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.run") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionRunEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.moved") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionMovedEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.behind") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionBehindEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.notice") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionNoticeEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.earlier") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionEarlierEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.capped") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionCappedEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.uncapped") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionUncappedEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.subagent") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionSubagentEvent>(&serde_json::Value::Object(body)))
            }
            Some("session.slate") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SessionSlateEvent>(&serde_json::Value::Object(body)))
            }
            Some("thread.waited") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<ThreadWaitedEvent>(&serde_json::Value::Object(body)))
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Word("object"));
        }
        if path.len() == 1 && path[0].as_str() == Some("type") {
            return Some(crate::types::check::Zod::Tags(
                "type",
                &[
                    "session.start",
                    "session.delta",
                    "session.done",
                    "session.end",
                    "session.steer",
                    "session.notify",
                    "session.permission",
                    "session.permission.closed",
                    "session.checkpoint",
                    "session.changes",
                    "session.plan",
                    "session.compacted",
                    "session.retry",
                    "session.context",
                    "session.run",
                    "session.moved",
                    "session.behind",
                    "session.notice",
                    "session.earlier",
                    "session.capped",
                    "session.uncapped",
                    "session.subagent",
                    "session.slate",
                    "thread.waited",
                ],
            ));
        }
        <SessionPermissionEvent as Check>::describe(path)
            .or_else(|| <SessionPermissionClosedEvent as Check>::describe(path))
            .or_else(|| <SessionCheckpointEvent as Check>::describe(path))
            .or_else(|| <SessionPlanEvent as Check>::describe(path))
            .or_else(|| <SessionCompactedEvent as Check>::describe(path))
            .or_else(|| <SessionRetryEvent as Check>::describe(path))
            .or_else(|| <SessionContextEvent as Check>::describe(path))
            .or_else(|| <SessionRunEvent as Check>::describe(path))
            .or_else(|| <SessionMovedEvent as Check>::describe(path))
            .or_else(|| <SessionBehindEvent as Check>::describe(path))
            .or_else(|| <SessionNoticeEvent as Check>::describe(path))
            .or_else(|| <SessionEarlierEvent as Check>::describe(path))
            .or_else(|| <SessionCappedEvent as Check>::describe(path))
            .or_else(|| <SessionUncappedEvent as Check>::describe(path))
            .or_else(|| <SessionSubagentEvent as Check>::describe(path))
            .or_else(|| <SessionSlateEvent as Check>::describe(path))
            .or_else(|| <ThreadWaitedEvent as Check>::describe(path))
    }
}

/// A stored event as a thread's head writes it: transcript-reader.ts `headShape` returns `{ ...e, text, cut, ...keptPatch }`, so
/// `cut` and then `patchCut` stand after every key the stored row held, where the schema (and `SessionEvent`) lists `cut` after
/// `line` and `patchCut` after `patch`. The text is the same length either way.
pub fn head_value(event: &SessionEvent) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = serde_json::to_value(event)?;
    if let Some(fields) = value.as_object_mut() {
        for key in ["cut", "patchCut"] {
            if let Some(kept) = fields.shift_remove(key) {
                fields.insert(key.to_string(), kept);
            }
        }
    }
    Ok(value)
}

pub fn head_text(event: &SessionEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(&crate::types::kept::Kept(head_value(event)?))
}

/// The events of a thread's head (`sessions.head` runs each through `headShape`, runtime/src/threads/sessions.ts:963), written
/// as a head writes them.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct HeadEvents(pub Vec<SessionEvent>);

impl std::ops::Deref for HeadEvents {
    type Target = Vec<SessionEvent>;

    fn deref(&self) -> &Vec<SessionEvent> {
        &self.0
    }
}

impl Serialize for HeadEvents {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{Error, SerializeSeq};
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for event in &self.0 {
            seq.serialize_element(&crate::types::kept::Kept(head_value(event).map_err(S::Error::custom)?))?;
        }
        seq.end()
    }
}

impl Check for HeadEvents {
    fn check(&self) -> Result<(), SchemaError> {
        self.0.check()
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<SchemaError>> {
        <Vec<SessionEvent> as Check>::pass(value)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        <Vec<SessionEvent> as Check>::describe(path)
    }
}
