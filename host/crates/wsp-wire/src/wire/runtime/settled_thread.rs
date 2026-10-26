// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// No refinement of its own; checks every nested field that has one.
impl Check for SettledThread {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.settled.check().map_err(|e| e.under("settled"))?;
        self.ended.check().map_err(|e| e.under("ended"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<SettledThread>>(obj, "settled", &mut out);
        crate::types::keys::req_key::<Vec<SessionSettleResultLeftItem>>(obj, "left", &mut out);
        crate::types::keys::opt_key::<Vec<ActivityEnded>>(obj, "ended", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResultLeftItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "why", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRestoreResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.restored.check().map_err(|e| e.under("restored"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<SettledThread>>(obj, "restored", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSearchHit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "snippet", &mut out);
        Some(out)
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `unsearched`: int min 0 exclusive
impl Check for SessionSearchResult {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.unsearched {
            num_above(v.as_f64(), 0.0).map_err(|e| e.under("unsearched"))?;
        }
        self.hits.check().map_err(|e| e.under("hits"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<SessionSearchHit>>(obj, "hits", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "unsearched", &mut out, Self::rule_unsearched);
        Some(out)
    }
}
impl SessionSearchResult {
    fn rule_unsearched(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_above(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAsideResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRewindResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "turns", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "files", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kept", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionStartResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.session.check().map_err(|e| e.under("session"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionView>(obj, "session", &mut out);
        crate::types::keys::req_key::<SessionStartOutcome>(obj, "outcome", &mut out);
        crate::types::keys::req_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "notice", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionWarmResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionWarmResultWarm>(obj, "warm", &mut out);
        Some(out)
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `running`: int min 0
/// - `limit`: int min 0
impl Check for ProviderKeyWorks {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.running.as_f64(), 0.0).map_err(|e| e.under("running"))?;
        num_at_least(self.limit.as_f64(), 0.0).map_err(|e| e.under("limit"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<True>(obj, "works", &mut out);
        crate::types::keys::req_key::<String>(obj, "account", &mut out);
        crate::types::keys::req_key::<String>(obj, "plan", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "running", &mut out, Self::rule_running);
        crate::types::keys::req_key_with::<JsInt>(obj, "limit", &mut out, Self::rule_limit);
        Some(out)
    }
}
impl ProviderKeyWorks {
    fn rule_running(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_limit(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BrokerStatus {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<BrokerStatusClaude>(obj, "claude", &mut out);
        crate::types::keys::opt_key::<BrokerStatusCodex>(obj, "codex", &mut out);
        crate::types::keys::opt_key::<String>(obj, "github", &mut out);
        Some(out)
    }
}

// Refinements on the variants of RuntimeOp (runtime_ops.rs), each an issue when it fails:
// - RuntimeOp::PairRedeem.code: string max 64
// - RuntimeOp::PairRedeem.name: string max 200
// - RuntimeOp::PlacesDoctor.doctorId: string max 64
// - RuntimeOp::PlacesDoctor.project: string max 200
// - RuntimeOp::PlacesSet.name: string max 200
// - RuntimeOp::PlacesSet.ssh: string max 300
// - RuntimeOp::PlacesSet.recipe: string min 1
// - RuntimeOp::PlacesSet.recipe: string max 200
// - RuntimeOp::PlacesSet.threads: int min 1
// - RuntimeOp::PlacesSet.machines: int min 1
// - RuntimeOp::PlacesSet.spendPerDayUsd: number min 0
// - RuntimeOp::PlacesSet.napMs: int min 60000
// - RuntimeOp::PlacesSet.napMs: int max 10800000
// - RuntimeOp::PlacesSet.napMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::PlacesSet.turnLimitMs: int min 3600000
// - RuntimeOp::PlacesSet.turnLimitMs: int max 86400000
// - RuntimeOp::PlacesSet.turnLimitMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::PlacesSet.deleteAfterMs: int min 0
// - RuntimeOp::PlacesSet.deleteAfterMs: int max 86400000
// - RuntimeOp::PlacesFollow.recipe: string min 1
// - RuntimeOp::PlacesFollow.recipe: string max 200
// - RuntimeOp::PlacesSkip.row: string min 1
// - RuntimeOp::PlacesSkip.row: string max 300
// - RuntimeOp::PlacesEstimate.ref: string max 200
// - RuntimeOp::PlacesAdd.addId: string max 64
// - RuntimeOp::PlacesAdd.address: string max 200
// - RuntimeOp::PlacesAdd.name: string max 200
// - RuntimeOp::PlacesAdd.sshPort: int min 1
// - RuntimeOp::PlacesAdd.sshPort: int max 65535
// - RuntimeOp::PlacesAdd.keyPath: string max 1024
// - RuntimeOp::PlacesAdd.hostKey: string max 200
// - RuntimeOp::PlacesAdd.recipe: string max 200
// - RuntimeOp::PlacesSetup.ref: string max 200
// - RuntimeOp::PlacesSetup.recipe: string max 200
// - RuntimeOp::PlacesSetup.addId: string max 64
// - RuntimeOp::PlacesSetup.row: string max 400
// - RuntimeOp::PlacesChoose.ref: string max 200
// - RuntimeOp::PlacesChoose.recipe: string max 200
// - RuntimeOp::RecipesGet.name: string max 200
// - RuntimeOp::RecipesSave.name: string max 200
// - RuntimeOp::RecipesSave.from: string max 200
// - RuntimeOp::RecipesRemove.name: string max 200
// - RuntimeOp::EventsSubscribe.after: int min 0
// - RuntimeOp::WorkspacesCreate.idleWindowMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesLook.theme: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesLook.glyph: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesViewed.blob: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesPullRequestSend.items: array min 1
// - RuntimeOp::WorkspacesPullRequestReply.replyTo: int min 0
// - RuntimeOp::WorkspacesPullRequestReply.body: string max 65536
// - RuntimeOp::WorkspacesMerge.head: string min 1
// - RuntimeOp::ActivityEnd.pid: int min 0 exclusive
// - RuntimeOp::SessionsStart.notify: array min 1
// - RuntimeOp::SessionsHistory.before: int min 0 exclusive
// - RuntimeOp::SessionsHistory.limit: int min 0 exclusive
// - RuntimeOp::SessionsHistory.limit: int max 1000
// - RuntimeOp::SessionsAttachment.index: int min 0
// - RuntimeOp::SessionsSettle.threadIds: array min 1
// - RuntimeOp::SessionsMark.threadIds: array min 1
// - RuntimeOp::SessionsRestore.threadIds: array min 1
// - RuntimeOp::WorkspacesPortReach.port: int min 1
// - RuntimeOp::WorkspacesPortReach.port: int max 65535
// - RuntimeOp::WorkspacesPortProbe.port: int min 1
// - RuntimeOp::WorkspacesPortProbe.port: int max 65535
// - RuntimeOp::WorkspacesExec.argv: array min 1
// - RuntimeOp::EditorOpen.line: int min 0 exclusive
// - RuntimeOp::ServersIcon.host: string min 1
// - RuntimeOp::ServersIcon.host: string max 260
// - RuntimeOp::AgentsSignInCode.code: string min 1
// - RuntimeOp::AgentsKey.key: string min 1
// - RuntimeOp::ProvidersCheckKey.provider: string max 64
// - RuntimeOp::ProvidersCheckKey.key: string min 1
// - RuntimeOp::AgentsSetup.program: string min 1
// - RuntimeOp::AgentsSetup.program: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::AgentsSetup.configDir: string min 1
// - RuntimeOp::AgentsSetup.configDir: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::AgentsSetup.args: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::SkillsSearch.limit: int min 1
// - RuntimeOp::SkillsSearch.limit: int max 50
// - RuntimeOp::InitKeys.provider: string max 64
// - RuntimeOp::ConversationsRead.agent: string min 1
// - RuntimeOp::ConversationsRead.conversation: string min 1
// - RuntimeOp::ConversationsRead.conversation: string max 200
// - RuntimeOp::ProjectsIcon.png: string max 327680
// - RuntimeOp::ProjectsIcons.hashes: array max 1000
// - RuntimeOp::ImageBuild.place: string min 1
// - RuntimeOp::ImageExport.dest: string min 1
// - RuntimeOp::ImageExport.passphrase: string min 12
// - RuntimeOp::ImageExport.passphrase: string max 256
// - RuntimeOp::SlatesEvent.requestId: string min 1
// - RuntimeOp::SlatesEvent.requestId: string max 200
// - RuntimeOp::SlatesEvent.rowAction: int min 0
// - RuntimeOp::SlatesResolve.paths: array max 200
// - RuntimeOp::SlatesImage.src: string max 4096
// - RuntimeOp::SlatesImage.have: string max 200

// runtime.ts:1035
// /** The secret values a request frame carries, read one level into a record and no deeper: a record of logins is as
//  * deep as a secret field goes, and the frame may be a stranger's. */
// export function requestSecrets(frame: unknown): string[] {
//   if (typeof frame !== "object" || frame === null) return [];
//   return SECRET_REQUEST_FIELDS.flatMap(field => {
//     const v = (frame as Record<string, unknown>)[field];
//     return typeof v === "string" ? [v] : typeof v === "object" && v !== null ? Object.values(v).filter((x): x is string => typeof x === "string") : [];
//   });
// }
pub fn request_secrets(frame: &Value) -> Vec<String> {
    let Some(map) = frame.as_object() else {
        return Vec::new();
    };
    SECRET_REQUEST_FIELDS
        .iter()
        .flat_map(|field| match map.get(*field) {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Object(inner)) => crate::types::kept::js_order(inner.iter().map(|(k, v)| (k.as_str(), v)))
                .into_iter()
                .filter_map(|(_, v)| v.as_str().map(str::to_string))
                .collect(),
            Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        })
        .collect()
}

// runtime.ts:1251
// /** The workspace an event is about, for the one reading every door that hides a workspace from a caller shares: the
//  * id on the event itself, else the id of the record or the status it carries. An event that names none is about
//  * this host rather than about any workspace, which is why a caller that may see only its own tree is sent none. */
// export function workspaceIdOf(event: unknown): string | undefined {
//   const e = event as { workspaceId?: unknown; workspace?: { id?: unknown }; status?: { id?: unknown }; forward?: { workspaceId?: unknown } };
//   for (const found of [e.workspaceId, e.workspace?.id, e.status?.id, e.forward?.workspaceId]) if (typeof found === "string") return found;
//   return undefined;
// }
pub fn workspace_id_of(event: &Value) -> Option<String> {
    [
        event.get("workspaceId"),
        event.get("workspace").and_then(|w| w.get("id")),
        event.get("status").and_then(|s| s.get("id")),
        event.get("forward").and_then(|f| f.get("workspaceId")),
    ]
    .into_iter()
    .flatten()
    .find_map(|v| v.as_str().map(str::to_string))
}

// runtime.ts:1260
// /** The thread an event is on behalf of, beside the reading above and for the same door: an event about a workspace
//  * that has no record yet is nobody's by the reading above, so the caller it was asked for by is named on it. */
// export function askerOf(event: unknown): EventAsker | undefined {
//   const asked = (event as { askedBy?: { threadId?: unknown; rootThreadId?: unknown } }).askedBy;
//   if (typeof asked?.threadId !== "string" || typeof asked.rootThreadId !== "string") return undefined;
//   return { threadId: asked.threadId, rootThreadId: asked.rootThreadId };
// }
pub fn asker_of(event: &Value) -> Option<EventAsker> {
    let asked = event.get("askedBy")?;
    let thread_id = asked.get("threadId")?.as_str()?;
    let root_thread_id = asked.get("rootThreadId")?.as_str()?;
    Some(EventAsker { thread_id: thread_id.to_string(), root_thread_id: root_thread_id.to_string() })
}

// runtime.ts:1318
// /** What a stop of one subagent the agent refused says, in the agent's own words; the line it stands in names the task. */
// export const taskStopRefusedLine = (agent: string, why: string): string => `${agent} would not stop it: ${why}`;
pub fn task_stop_refused_line(agent: &str, why: &str) -> String {
    format!("{agent} would not stop it: {why}")
}

// runtime.ts:1320
// /** What a stop of one subagent says where the agent offers none. */
// export const taskStopUnsupportedLine = (agent: string): string => `Stop is not available for ${agent} subagents; stop the thread to stop them all`;
pub fn task_stop_unsupported_line(agent: &str) -> String {
    format!("Stop is not available for {agent} subagents; stop the thread to stop them all")
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for WorkspacesReviewDraftVerdict {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for AgentsKeyWay {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for AgentsCloudSignInAgent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SlatesEventEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SlatesApproveScope {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionInterruptOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionSteerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionAccessOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionAnswerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionRenameOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionStartOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for SessionWarmResultWarm {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for BrokerStatusClaude {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl Check for BrokerStatusCodex {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
