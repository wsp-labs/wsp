// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/session-events.ts:384 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnChangedFile {
    pub path: String,
    pub kind: String,
    pub additions: JsNumber,
    pub deletions: JsNumber,
}
impl Check for TurnChangedFile {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "kind", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "additions", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "deletions", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "kind" => <String as Check>::describe(rest),
                "additions" => <JsNumber as Check>::describe(rest),
                "deletions" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:395 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionChangesEvent {
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
    pub files: Vec<TurnChangedFile>,
    pub moved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shared: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub others: Option<Vec<TurnChangedFile>>,
}
impl SessionChangesEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.changes";
}
impl Check for SessionChangesEvent {
    /// Rules: pos: > 0.
    /// Nested checks: files, others.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.files.check().map_err(|e| e.under("files"))?;
        self.others.check().map_err(|e| e.under("others"))?;
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
        crate::types::keys::req_key::<Vec<TurnChangedFile>>(obj, "files", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "moved", &mut out);
        crate::types::keys::opt_key::<True>(obj, "shared", &mut out);
        crate::types::keys::opt_key::<Vec<TurnChangedFile>>(obj, "others", &mut out);
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
                "files" => <Vec<TurnChangedFile> as Check>::describe(rest),
                "moved" => <Vec<String> as Check>::describe(rest),
                "shared" => None,
                "others" => <Option<Vec<TurnChangedFile>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionChangesEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanStepState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "done")]
    Done,
}
impl Check for PlanStepState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["pending", "working", "done"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:408 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub text: String,
    pub state: PlanStepState,
}
impl Check for PlanStep {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        crate::types::keys::req_key::<PlanStepState>(obj, "state", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "text" => <String as Check>::describe(rest),
                "state" => <PlanStepState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// views/session-events.ts:321 at a3b870a6e
// export const agentStartingLine = (agent: string, installs: boolean): string => (installs ? `${agent} ${FIRST_RUN_WORD}` : `Starting ${agent}`);
pub fn agent_starting_line(agent: &str, installs: bool) -> String {
    if installs {
        format!("{agent} {FIRST_RUN_WORD}")
    } else {
        format!("Starting {agent}")
    }
}

/// What `host_from_env` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFromEnv {
    pub url: String,
    pub token: String,
    pub host_key: Option<String>,
}

// views/session-events.ts:334 at a3b870a6e
// export function hostFromEnv(env: Readonly<Record<string, string | undefined>>): { url: string; token: string; hostKey?: string } | undefined {
//   const url = env[HOST_URL_ENV]?.trim();
//   const token = env[HOST_TOKEN_ENV]?.trim();
//   const hostKey = env[HOST_KEY_ENV]?.trim();
//   if (url === undefined || url === "" || token === undefined || token === "") return undefined;
//   return { url, token, ...(hostKey === undefined || hostKey === "" ? {} : { hostKey }) };
// }
/// The host a process inside a turn reaches: HOST_URL_ENV, HOST_TOKEN_ENV and HOST_KEY_ENV, each trimmed as
/// JavaScript's `trim` does; None where the url or the token is missing or empty, and no key where it is.
pub fn host_from_env(env: &HashMap<String, String>) -> Option<HostFromEnv> {
    let trimmed = |name: &str| env.get(name).map(|v| js_trim(v).to_string());
    let url = trimmed(HOST_URL_ENV).filter(|v| !v.is_empty())?;
    let token = trimmed(HOST_TOKEN_ENV).filter(|v| !v.is_empty())?;
    let host_key = trimmed(HOST_KEY_ENV).filter(|v| !v.is_empty());
    Some(HostFromEnv { url, token, host_key })
}

// views/session-events.ts:355 at a3b870a6e
// export const wspPackageIn = (dir: string): string => `${dir}/wsp`;
pub fn wsp_package_in(dir: &str) -> String {
    format!("{dir}/wsp")
}

// views/session-events.ts:356 at a3b870a6e
// export const wspBinIn = (dir: string): string => `${wspPackageIn(dir)}/dist/bin.js`;
pub fn wsp_bin_in(dir: &str) -> String {
    format!("{}/dist/bin.js", wsp_package_in(dir))
}

// views/session-events.ts:365 at a3b870a6e
// export function turnTokenOf(env: Readonly<Record<string, string | undefined>>): string | undefined {
//   const token = env[TURN_TOKEN_ENV];
//   return token === undefined || token === "" ? undefined : token;
// }
/// TURN_TOKEN_ENV as it is, untrimmed; None where it is missing or empty.
pub fn turn_token_of(env: &HashMap<String, String>) -> Option<String> {
    env.get(TURN_TOKEN_ENV).filter(|token| !token.is_empty()).cloned()
}

// views/session-events.ts:665 at a3b870a6e
// export const copiedFromOf = (e: SessionEvent): string | undefined => ("copiedFrom" in e ? e.copiedFrom : undefined);
/// The `copiedFrom` of an event, None where it has none or its type (`session.slate`) carries no such key.
pub fn copied_from_of(e: &SessionEvent) -> Option<String> {
    match e {
        SessionEvent::SessionSubagentEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionPlanEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionCompactedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionRetryEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionContextEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionPermissionEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionPermissionClosedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionCheckpointEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionMovedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionBehindEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionNoticeEvent(e) => e.copied_from.clone(),
        SessionEvent::ThreadWaitedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionEarlierEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionCappedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionUncappedEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionRunEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionStartEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionDeltaEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionDoneEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionEndEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionSteerEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionNotifyEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionChangesEvent(e) => e.copied_from.clone(),
        SessionEvent::SessionSlateEvent(_) => None,
    }
}

// views/session-events.ts:673 at a3b870a6e
// export const isSessionEvent = (e: { type: string }): e is SessionEvent => SESSION_EVENT_TYPES.has(e.type as SessionEvent["type"]);
/// Whether an event type is one of SESSION_EVENT_TYPES.
pub fn is_session_event(event_type: &str) -> bool {
    SESSION_EVENT_TYPES.contains(&event_type)
}
