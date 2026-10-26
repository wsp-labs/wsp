// SPDX-License-Identifier: AGPL-3.0-only
//! A Codex plan's banked resets, read and spent through `codex app-server` outside any turn. TS
//! adapter-codex/src/reset.ts. Spec: spec/codex_limits.md rules 7-12.

use serde_json::{Map, Value};

use super::command::{build_env, slug, CodexEnvOptions};
use super::limits::{credits_of, limit_of, snapshot_of, LimitAccount};
use crate::pending::{
    answers_of, app_server_initialize, app_server_notification, app_server_request, app_server_script, result_of, ServerAnswer, ServerStage,
};
use crate::port::{program_word, AdapterError, HarnessLimit, PlanResets, ResetReading, ResetRoad, ResetSpend};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::usage::HarnessLimitAccount;

const INIT: i64 = 1;
const ACCOUNT: i64 = 2;
const LIMITS: i64 = 3;
const CONSUME: i64 = 4;
const METHOD_NOT_FOUND: f64 = -32601.0;

/// `cd ~` and the exports first: the exec a computer runs this under carries no environment of its own. TS reset.ts:23.
fn script(road: &ResetRoad, stages: &[ServerStage]) -> Result<String, String> {
    let codex = program_word("codex", road.launch.as_ref());
    let env = build_env(&CodexEnvOptions { base: road.env.clone(), home: road.home.clone(), api_key: None, broker: None })?;
    let exports: Vec<String> = env.iter().map(|(k, v)| format!("{k}={}", shell_quote(v))).collect();
    Ok(format!("cd ~ && export {}\n{}", exports.join(" "), app_server_script(&codex, stages)))
}

/// The lines every reset script opens with: initialize and the initialized notification. TS reset.ts:17.
fn open_lines() -> Vec<String> {
    vec![app_server_initialize(INIT), app_server_notification("initialized")]
}

fn limits_read() -> String {
    app_server_request(LIMITS, "account/rateLimits/read", Map::new())
}

/// TS reset.ts:342.
pub fn reset_read_command(road: &ResetRoad) -> Result<String, String> {
    let mut lines = open_lines();
    lines.push(app_server_request(ACCOUNT, "account/read", Map::new()));
    lines.push(limits_read());
    script(road, &[ServerStage { lines, answers: 3 }])
}

/// TS reset.ts:346.
pub fn reset_credit_command(road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, String> {
    let mut params = Map::new();
    params.insert("idempotencyKey".into(), Value::String(slug("idempotencyKey", idempotency_key)?));
    if let Some(credit) = credit_id {
        params.insert("creditId".into(), Value::String(slug("creditId", credit)?));
    }
    let mut consume = open_lines();
    consume.push(app_server_request(CONSUME, "account/rateLimitResetCredit/consume", params));
    script(road, &[ServerStage { lines: consume, answers: 2 }, ServerStage { lines: vec![limits_read()], answers: 1 }])
}

/// A string that is not empty, the reset reader's own refusal of "". TS reset.ts:50.
fn nonempty(value: Option<&Value>) -> Option<&str> {
    value?.as_str().filter(|s| !s.is_empty())
}

/// The limits answer as a reading, with the sign-in account/read named where it was asked. TS reset.ts:50.
fn limit_in(answers: &indexmap::IndexMap<i64, ServerAnswer>, sign_in: Option<&Map<String, Value>>) -> Option<HarnessLimit> {
    let answer = result_of(answers, LIMITS)?;
    let email = nonempty(sign_in.and_then(|s| s.get("email")));
    let plan = nonempty(sign_in.and_then(|s| s.get("planType")));
    let id = nonempty(answer.get("accountId")).or(email);
    let account = LimitAccount { id: id.map(String::from), label: email.map(String::from), plan: plan.map(String::from) };
    let credits = credits_of(Some(answer));
    let snapshot = snapshot_of(Some(answer)).unwrap_or_default();
    Some(limit_of(&snapshot, &account, credits.clone()).unwrap_or_else(|| HarnessLimit {
        windows: Vec::new(),
        plan: None,
        status: None,
        account: id.map(|id| HarnessLimitAccount { id: id.to_string(), label: email.map(String::from) }),
        keyed: None,
        credits,
    }))
}

/// `None` where the server never answered initialize. TS reset.ts:375.
pub fn parse_reset_read(stdout: &str) -> Option<ResetReading> {
    let answers = answers_of(stdout);
    if !answers.contains_key(&INIT) {
        return None;
    }
    let sign_in = result_of(&answers, ACCOUNT).and_then(|r| r.get("account")).and_then(Value::as_object);
    if sign_in.and_then(|s| s.get("type")).and_then(Value::as_str) == Some("apiKey") {
        return Some(ResetReading { keyed: true, limit: None });
    }
    Some(ResetReading { keyed: false, limit: limit_in(&answers, sign_in) })
}

/// TS reset.ts:385.
pub fn parse_reset_credit(stdout: &str) -> ResetSpend {
    let answers = answers_of(stdout);
    match answers.get(&CONSUME) {
        None => ResetSpend::Unanswered,
        Some(ServerAnswer::Error { code, message, .. }) => {
            ResetSpend::Refused { refused: message.clone(), too_old: *code == Some(METHOD_NOT_FOUND) }
        }
        Some(ServerAnswer::Result(result)) => ResetSpend::Outcome {
            outcome: nonempty(result.get("outcome")).unwrap_or("unknown").to_string(),
            limit: limit_in(&answers, None),
        },
    }
}

/// Finishes "<agent> on <computer> ..." for a build that answers no resets. TS reset.ts:399.
pub const TOO_OLD: &str = "is older than 0.141 and has no resets";

/// The agent's one registered set of reset roads. TS reset.ts:394.
pub struct CodexPlanResets;

impl PlanResets for CodexPlanResets {
    /// The port's method answers no error where the TS throws (a relative home, a key that is no slug): this panics
    /// with the refusal's words there. Asked of the port's owner to answer `Result`.
    fn read_command(&self, road: &ResetRoad) -> Result<String, AdapterError> {
        reset_read_command(road).map_err(AdapterError)
    }
    fn parse_read(&self, stdout: &str) -> Option<ResetReading> {
        parse_reset_read(stdout)
    }
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, AdapterError> {
        reset_credit_command(road, idempotency_key, credit_id).map_err(AdapterError)
    }
    fn parse_spend(&self, stdout: &str) -> ResetSpend {
        parse_reset_credit(stdout)
    }
    fn too_old(&self) -> &str {
        TOO_OLD
    }
}
