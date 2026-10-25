// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/session-events.ts at a3b870a6e, apart only for the repo's file-size law; session_events.rs re-exports them.

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

use super::session_events::*;
use crate::literal;
use crate::types::attachments::AttachmentRecord;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::slate_wire::SessionSlateEvent;
use crate::types::usage::TurnLimit;
use crate::types::usage::UsageTokens;
use crate::views::session::PermissionOutcome;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionView;
use crate::views::session::SubagentState;
use crate::views::session::SubagentUsage;
use crate::views::session::TurnRefusal;
use crate::wire::helpers::PermissionOption;
use wsp_frames::True;

/// TS: packages/protocol/src/views/session-events.ts:415 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSubagentEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub task: String,
    pub state: SubagentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<SubagentUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

impl SessionSubagentEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.subagent";
}

impl Check for SessionSubagentEvent {
    /// Rules: pos: > 0; depth: > 0; line: > 0.
    /// Nested checks: state, usage.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.depth {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("depth"))?;
        }
        self.usage.check().map_err(|e| e.under("usage"))?;
        if let Some(v) = self.line {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("line"))?;
        }
        Ok(())
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
                "task" => <String as Check>::describe(rest),
                "state" => <SubagentState as Check>::describe(rest),
                "parentToolUseId" => <Option<String> as Check>::describe(rest),
                "title" => <Option<String> as Check>::describe(rest),
                "summary" => <Option<String> as Check>::describe(rest),
                "depth" => <Option<i64> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "asked" => <Option<String> as Check>::describe(rest),
                "usage" => <Option<SubagentUsage> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:438 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<PlanStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl SessionPlanEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.plan";
}

impl Check for SessionPlanEvent {
    /// Rules: pos: > 0.
    /// Nested checks: steps.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "steps" => None,
                "text" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:449 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompactedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

impl SessionCompactedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.compacted";
}

impl Check for SessionCompactedEvent {
    /// Rules: pos: > 0; line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        if let Some(v) = self.line {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("line"))?;
        }
        Ok(())
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
                "before" => <Option<JsNumber> as Check>::describe(rest),
                "after" => <Option<JsNumber> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:461 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRetryEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub attempt: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

impl SessionRetryEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.retry";
}

impl Check for SessionRetryEvent {
    /// Rules: pos: > 0; attempt: > 0; of: > 0; line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        crate::types::check::num_above(self.attempt as f64, 0.0).map_err(|e| e.under("attempt"))?;
        if let Some(v) = self.of {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("of"))?;
        }
        if let Some(v) = self.line {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("line"))?;
        }
        Ok(())
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
                "attempt" => <i64 as Check>::describe(rest),
                "of" => <Option<i64> as Check>::describe(rest),
                "why" => <Option<String> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:475 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub context: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<JsNumber>,
}

impl SessionContextEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.context";
}

impl Check for SessionContextEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "context" => <JsNumber as Check>::describe(rest),
                "window" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:486 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPermissionEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub options: Vec<PermissionOption>,
}

impl SessionPermissionEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.permission";
}

impl Check for SessionPermissionEvent {
    /// Rules: pos: > 0.
    /// Nested checks: options.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.options.check().map_err(|e| e.under("options"))?;
        Ok(())
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
                "askId" => <String as Check>::describe(rest),
                "toolName" => <String as Check>::describe(rest),
                "toolUseId" => <Option<String> as Check>::describe(rest),
                "parentToolUseId" => <Option<String> as Check>::describe(rest),
                "input" => <String as Check>::describe(rest),
                "detail" => <Option<String> as Check>::describe(rest),
                "options" => <Vec<PermissionOption> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:495 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPermissionClosedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub ask_id: String,
    pub outcome: PermissionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
}

impl SessionPermissionClosedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.permission.closed";
}

impl Check for SessionPermissionClosedEvent {
    /// Rules: pos: > 0.
    /// Nested checks: outcome.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        Ok(())
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
                "askId" => <String as Check>::describe(rest),
                "outcome" => <PermissionOutcome as Check>::describe(rest),
                "optionId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:509 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCheckpointEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
}

impl SessionCheckpointEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.checkpoint";
}

impl Check for SessionCheckpointEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "ref" => <Option<String> as Check>::describe(rest),
                "based" => None,
                "anchor" => <Option<String> as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:525 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMovedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fresh: Option<True>,
}

impl SessionMovedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.moved";
}

impl Check for SessionMovedEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "from" => <String as Check>::describe(rest),
                "to" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "fresh" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:538 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBehindEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub text: String,
}

impl SessionBehindEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.behind";
}

impl Check for SessionBehindEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:547 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNoticeEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub text: String,
}

impl SessionNoticeEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.notice";
}

impl Check for SessionNoticeEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
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
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:556 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadWaitedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

impl ThreadWaitedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "thread.waited";
}

impl Check for ThreadWaitedEvent {
    /// Rules: pos: > 0; ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        crate::types::check::num_at_least(self.ms as f64, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionEarlierEventWho {
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "note")]
    Note,
}

impl Check for SessionEarlierEventWho {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["person", "agent", "tool", "note"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:568 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEarlierEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub who: SessionEarlierEventWho,
    pub text: String,
}

impl SessionEarlierEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.earlier";
}

impl Check for SessionEarlierEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.who.check().map_err(|e| e.under("who"))?;
        Ok(())
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
                "who" => <SessionEarlierEventWho as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionCappedEventWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}

impl Check for SessionCappedEventWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["slot", "minute"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:579 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCappedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub place_id: String,
    pub place: String,
    pub running: i64,
    pub at_once: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<SessionCappedEventWhy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub why: SessionUncappedEventWhy,
    pub at_once: i64,
}

impl SessionUncappedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.uncapped";
}

impl Check for SessionUncappedEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub run_id: String,
    pub block: String,
    pub command: String,
    pub state: RunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pty_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
            crate::types::check::num_above(v as f64, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
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

/// TS: packages/protocol/src/views/session-events.ts:625 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub run_id: String,
    pub block: String,
    pub command: String,
    pub state: RunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pty_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Word("object")); }
        if path.len() == 1 && path[0].as_str() == Some("type") { return Some(crate::types::check::Zod::Tags("type", &["session.start", "session.delta", "session.done", "session.end", "session.steer", "session.notify", "session.permission", "session.permission.closed", "session.checkpoint", "session.changes", "session.plan", "session.compacted", "session.retry", "session.context", "session.run", "session.moved", "session.behind", "session.notice", "session.earlier", "session.capped", "session.uncapped", "session.subagent", "session.slate", "thread.waited"])); }
        <SessionPermissionEvent as Check>::describe(path).or_else(|| <SessionPermissionClosedEvent as Check>::describe(path)).or_else(|| <SessionCheckpointEvent as Check>::describe(path)).or_else(|| <SessionPlanEvent as Check>::describe(path)).or_else(|| <SessionCompactedEvent as Check>::describe(path)).or_else(|| <SessionRetryEvent as Check>::describe(path)).or_else(|| <SessionContextEvent as Check>::describe(path)).or_else(|| <SessionRunEvent as Check>::describe(path)).or_else(|| <SessionMovedEvent as Check>::describe(path)).or_else(|| <SessionBehindEvent as Check>::describe(path)).or_else(|| <SessionNoticeEvent as Check>::describe(path)).or_else(|| <SessionEarlierEvent as Check>::describe(path)).or_else(|| <SessionCappedEvent as Check>::describe(path)).or_else(|| <SessionUncappedEvent as Check>::describe(path)).or_else(|| <SessionSubagentEvent as Check>::describe(path)).or_else(|| <SessionSlateEvent as Check>::describe(path)).or_else(|| <ThreadWaitedEvent as Check>::describe(path))
    }
}
