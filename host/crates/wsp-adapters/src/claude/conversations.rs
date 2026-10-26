// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's own conversations on a computer. TS: packages/adapter-claude/src/conversations.ts at a3b870a6e.

use std::sync::Arc;

use futures::future::{join_all, FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use crate::claude::landmines::{build_env, BaseEnv, ClaudeEnvOptions};
use crate::pending::{claude_project_key, folders_stamp_command, shell_quote};
use crate::port::{
    program_word, AgentLaunch, ConversationEarlier, ConversationLine, ConversationOrigin, ConversationRoad, ConversationStore, EarlierAsk,
    EarlierRead, Env, ExecFailure, StoreError, StoredConversation,
};
use wsp_wire::types::check::parse;
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_wire::wire::limits::TRANSCRIPT_DIRS_MAX;
use wsp_wire::wire::machine_replies::{TranscriptRow, TranscriptVoice, TranscriptsListReply, TranscriptsReadReply};
use wsp_words::js::js_trim;
use wsp_words::v8_json::parse_json_lax;

/// conversations.ts:24.
pub const PROGRAMMATIC_ENTRYPOINTS: [&str; 3] = ["sdk-cli", "sdk-ts", "sdk-py"];

/// conversations.ts:27.
pub fn origin_of(entrypoint: Option<&str>) -> ConversationOrigin {
    match entrypoint {
        Some(entrypoint) if PROGRAMMATIC_ENTRYPOINTS.contains(&entrypoint) => ConversationOrigin::Script,
        _ => ConversationOrigin::Terminal,
    }
}

/// conversations.ts:31. Each cwd's key, then `pinned`, each once, first place kept.
pub fn keys_for(cwds: &[String], pinned: Option<&str>) -> Vec<String> {
    let mut keys: IndexSet<String> = cwds.iter().map(|cwd| claude_project_key(cwd)).collect();
    if let Some(pinned) = pinned {
        keys.insert(pinned.to_string());
    }
    keys.into_iter().collect()
}

/// conversations.ts:34.
pub fn present_command(root: &str, keys: &[String], file: Option<&str>) -> String {
    let test = match file {
        None => "-d \"$d\"".to_string(),
        Some(file) => format!("-f \"$d\"/{}", shell_quote(file)),
    };
    let words: Vec<String> = keys.iter().map(|key| shell_quote(key)).collect();
    format!("cd {} 2>/dev/null && for d in {}; do [ {test} ] && printf '%s\\n' \"$d\"; done; true", shell_quote(root), words.join(" "))
}

/// conversations.ts:38. Runs of at most TRANSCRIPT_DIRS_MAX (64); an empty list gives no runs.
pub fn runs_of<T: Clone>(list: &[T]) -> Vec<Vec<T>> {
    list.chunks(TRANSCRIPT_DIRS_MAX as usize).map(<[T]>::to_vec).collect()
}

/// conversations.ts:49.
pub fn live_command(launch: Option<&AgentLaunch>) -> String {
    format!(
        "cd ~ && unset ${{!CLAUDE_CODE_@}} CLAUDECODE FORCE_CODE_TERMINAL; {ENV_FROM_INPUT}; {} agents --json",
        program_word("claude", launch)
    )
}

/// conversations.ts:55. `None` where stdout is not a JSON array.
pub fn parse_live(stdout: &str) -> Option<IndexSet<String>> {
    let Value::Array(rows) = parse_json_lax(js_trim(stdout))? else { return None };
    Some(rows.iter().filter_map(|row| row.as_object()?.get("sessionId")?.as_str().filter(|id| !id.is_empty()).map(String::from)).collect())
}

/// What claude_conversations takes. conversations.ts:71.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationDeps {
    pub config_dir: String,
    pub project_dir_name: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub base_env: Option<BaseEnv>,
}

/// conversations.ts:71. Answers a store with list, stamp, live and earlier, and no lets_go.
pub fn claude_conversations(deps: ConversationDeps) -> Arc<dyn ConversationStore> {
    let root = format!("{}/projects", deps.config_dir.trim_end_matches('/'));
    let env = build_env(&ClaudeEnvOptions { base: deps.base_env, ..Default::default() });
    Arc::new(ClaudeStore { root, pinned: deps.project_dir_name, launch: deps.launch, env })
}

struct ClaudeStore {
    root: String,
    pinned: Option<String>,
    launch: Option<AgentLaunch>,
    env: Env,
}

impl ConversationStore for ClaudeStore {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        let (root, pinned) = (self.root.clone(), self.pinned.clone());
        async move { list_rows(&root, pinned.as_deref(), &cwds, &road).await }.boxed_local()
    }

    fn live(&self, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Option<IndexSet<String>>>> {
        let command = live_command(self.launch.as_ref());
        let env = self.env.clone();
        Some(async move { road.exec.exec(&command, Some(&env)).await.ok().and_then(|out| parse_live(&out)) }.boxed_local())
    }

    fn stamp(&self, cwds: Vec<String>, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Result<String, StoreError>>> {
        let folders: Vec<String> =
            keys_for(&cwds, self.pinned.as_deref()).iter().map(|key| shell_quote(&format!("{}/{key}", self.root))).collect();
        let command = folders_stamp_command(&folders);
        Some(async move { road.exec.exec(&command, None).await.map_err(exec_error) }.boxed_local())
    }

    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        let (root, pinned) = (self.root.clone(), self.pinned.clone());
        async move { earlier_read(&root, pinned.as_deref(), &id, &ask, &road).await }.boxed_local()
    }
}

fn exec_error(error: ExecFailure) -> StoreError {
    StoreError { message: error.message, lasting: false }
}

fn schema_error(error: wsp_wire::types::check::SchemaError) -> StoreError {
    StoreError { message: error.issue_text(), lasting: false }
}

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// The keys a frame names: all of them where one frame takes them, else only those the store holds.
async fn present_keys(root: &str, keys: Vec<String>, road: &ConversationRoad, file: Option<&str>) -> Result<Vec<String>, StoreError> {
    if keys.len() <= TRANSCRIPT_DIRS_MAX as usize {
        return Ok(keys);
    }
    let held = road.exec.exec(&present_command(root, &keys, file), None).await.map_err(exec_error)?;
    let held: IndexSet<&str> = held.split('\n').collect();
    Ok(keys.into_iter().filter(|key| held.contains(key.as_str())).collect())
}

async fn list_rows(
    root: &str,
    pinned: Option<&str>,
    cwds: &[String],
    road: &ConversationRoad,
) -> Result<Vec<StoredConversation>, StoreError> {
    let dirs = present_keys(root, keys_for(cwds, pinned), road, None).await?;
    let mut frames = Vec::new();
    for dir_run in runs_of(&dirs) {
        for cwd_run in runs_of(cwds) {
            frames.push(object(json!({"op": "transcripts.list", "root": root, "dirs": dir_run, "cwds": cwd_run})));
        }
    }
    let replies = join_all(frames.into_iter().map(|frame| (road.ask)(frame))).await;
    let mut newest: IndexMap<String, TranscriptRow> = IndexMap::new();
    for reply in replies {
        let reply: TranscriptsListReply = parse(&Value::Object(reply.map_err(exec_error)?)).map_err(schema_error)?;
        for row in reply.rows {
            let known = newest.get(&row.id).map_or(f64::NEG_INFINITY, |old| old.last_at.0);
            if known < row.last_at.0 {
                newest.insert(row.id.clone(), row);
            }
        }
    }
    Ok(newest
        .into_values()
        .map(|row| StoredConversation {
            id: row.id.clone(),
            title: row.title.clone().or_else(|| row.first_prompt.clone()).unwrap_or_else(|| row.id.clone()),
            first_prompt: row.first_prompt,
            branch: row.branch,
            cwd: row.cwd,
            last_at: row.last_at.0,
            bytes: Some(row.bytes.0),
            origin: origin_of(row.entrypoint.as_deref()),
        })
        .collect())
}

fn voice_word(who: TranscriptVoice) -> &'static str {
    match who {
        TranscriptVoice::Person => "person",
        TranscriptVoice::Agent => "agent",
        TranscriptVoice::Tool => "tool",
    }
}

async fn earlier_read(
    root: &str,
    pinned: Option<&str>,
    id: &str,
    ask: &EarlierAsk,
    road: &ConversationRoad,
) -> Result<EarlierRead, StoreError> {
    let dirs = present_keys(root, keys_for(&ask.cwds, pinned), road, Some(&format!("{id}.jsonl"))).await?;
    let frames = runs_of(&dirs).into_iter().map(|dir_run| {
        let mut frame = object(json!({"op": "transcripts.read", "root": root, "dirs": dir_run, "session": id, "last": ask.last}));
        if let Some(tail) = ask.tail {
            frame.insert("tail".into(), json!(tail));
        }
        frame
    });
    let replies = join_all(frames.map(|frame| (road.ask)(frame))).await;
    let mut found = None;
    for reply in replies {
        let reply: TranscriptsReadReply = parse(&Value::Object(reply.map_err(exec_error)?)).map_err(schema_error)?;
        if reply.found && found.is_none() {
            found = Some(reply);
        }
    }
    let Some(reply) = found else { return Ok(EarlierRead::Gone) };
    Ok(EarlierRead::Read(ConversationEarlier {
        cwd: reply.cwd,
        title: reply.title,
        lines: reply
            .messages
            .into_iter()
            .map(|message| ConversationLine {
                who: voice_word(message.who).to_string(),
                text: message.text,
                tool: message.tool,
                at: message.at.map(|at| at.0),
            })
            .collect(),
        earlier: reply.earlier.0 as u32,
    }))
}
