// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/session-events.ts:415 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSubagentEvent {
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
    pub task: String,
    pub state: SubagentState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub depth: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub usage: Option<SubagentUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.depth {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("depth"))?;
        }
        self.usage.check().map_err(|e| e.under("usage"))?;
        if let Some(v) = self.line {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("line"))?;
        }
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
        crate::types::keys::req_key::<String>(obj, "task", &mut out);
        crate::types::keys::req_key::<SubagentState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentToolUseId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "title", &mut out);
        crate::types::keys::opt_key::<String>(obj, "summary", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "depth", &mut out, Self::rule_depth);
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "asked", &mut out);
        crate::types::keys::opt_key::<SubagentUsage>(obj, "usage", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
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
impl SessionSubagentEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_line(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:438 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanEvent {
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub steps: Option<Vec<PlanStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.steps.check().map_err(|e| e.under("steps"))?;
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
        crate::types::keys::opt_key::<Vec<PlanStep>>(obj, "steps", &mut out);
        crate::types::keys::opt_key::<String>(obj, "text", &mut out);
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
                "steps" => <Option<Vec<PlanStep>> as Check>::describe(rest),
                "text" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionPlanEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:449 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompactedEvent {
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub before: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub after: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
}
impl SessionCompactedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.compacted";
}
impl Check for SessionCompactedEvent {
    /// Rules: pos: > 0; line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        if let Some(v) = self.line {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("line"))?;
        }
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
        crate::types::keys::opt_key::<JsNumber>(obj, "before", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "after", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
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
                "before" => <Option<JsNumber> as Check>::describe(rest),
                "after" => <Option<JsNumber> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionCompactedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_line(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:461 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRetryEvent {
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
    pub attempt: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub of: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
}
impl SessionRetryEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.retry";
}
impl Check for SessionRetryEvent {
    /// Rules: pos: > 0; attempt: > 0; of: > 0; line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        crate::types::check::num_above(self.attempt.0 .0, 0.0).map_err(|e| e.under("attempt"))?;
        if let Some(v) = self.of {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("of"))?;
        }
        if let Some(v) = self.line {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("line"))?;
        }
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
        crate::types::keys::req_key_with::<JsInt>(obj, "attempt", &mut out, Self::rule_attempt);
        crate::types::keys::opt_key_with::<JsInt>(obj, "of", &mut out, Self::rule_of);
        crate::types::keys::opt_key::<String>(obj, "why", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
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
                "attempt" => <i64 as Check>::describe(rest),
                "of" => <Option<i64> as Check>::describe(rest),
                "why" => <Option<String> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionRetryEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_attempt(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_of(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_line(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:475 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextEvent {
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
    pub context: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
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
        crate::types::keys::req_key::<JsNumber>(obj, "context", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "window", &mut out);
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
                "context" => <JsNumber as Check>::describe(rest),
                "window" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionContextEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:486 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPermissionEvent {
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
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_tool_use_id: Option<String>,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.options.check().map_err(|e| e.under("options"))?;
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
        crate::types::keys::req_key::<String>(obj, "askId", &mut out);
        crate::types::keys::req_key::<String>(obj, "toolName", &mut out);
        crate::types::keys::opt_key::<String>(obj, "toolUseId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentToolUseId", &mut out);
        crate::types::keys::req_key::<String>(obj, "input", &mut out);
        crate::types::keys::opt_key::<String>(obj, "detail", &mut out);
        crate::types::keys::req_key::<Vec<PermissionOption>>(obj, "options", &mut out);
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
impl SessionPermissionEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:495 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPermissionClosedEvent {
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
    pub ask_id: String,
    pub outcome: PermissionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.outcome.check().map_err(|e| e.under("outcome"))?;
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
        crate::types::keys::req_key::<String>(obj, "askId", &mut out);
        crate::types::keys::req_key::<PermissionOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<String>(obj, "optionId", &mut out);
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
                "askId" => <String as Check>::describe(rest),
                "outcome" => <PermissionOutcome as Check>::describe(rest),
                "optionId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionPermissionClosedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:509 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCheckpointEvent {
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub r#ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub based: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
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
        crate::types::keys::opt_key::<String>(obj, "ref", &mut out);
        crate::types::keys::opt_key::<True>(obj, "based", &mut out);
        crate::types::keys::opt_key::<String>(obj, "anchor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kept", &mut out);
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
                "ref" => <Option<String> as Check>::describe(rest),
                "based" => None,
                "anchor" => <Option<String> as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionCheckpointEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:525 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMovedEvent {
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
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
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
        crate::types::keys::req_key::<String>(obj, "from", &mut out);
        crate::types::keys::req_key::<String>(obj, "to", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<True>(obj, "fresh", &mut out);
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
                "from" => <String as Check>::describe(rest),
                "to" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "fresh" => None,
                _ => None,
            },
        }
    }
}
impl SessionMovedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:538 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBehindEvent {
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
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
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
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
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionBehindEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:547 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNoticeEvent {
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
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
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
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
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionNoticeEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:556 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadWaitedEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub ms: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        crate::types::check::num_at_least(self.ms.0 .0, 0.0).map_err(|e| e.under("ms"))?;
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
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::opt_key::<String>(obj, "copiedFrom", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
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
impl ThreadWaitedEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.who.check().map_err(|e| e.under("who"))?;
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
        crate::types::keys::req_key::<SessionEarlierEventWho>(obj, "who", &mut out);
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
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
                "who" => <SessionEarlierEventWho as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionEarlierEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
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
