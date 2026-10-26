// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's own threads on a computer, read through `codex app-server` there. TS adapter-codex/src/conversations.ts and
//! the store at adapter.ts:1331-1388. Spec: spec/codex_conversations.md.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use super::command::{codex_word, slug, CodexBroker};
use super::fields::{count, rec};
use super::items::tool_call_of;
use crate::pending::{
    answers_of, app_server_initialize, app_server_notification, app_server_request, app_server_script, folders_stamp_command, shell_quote,
    ServerAnswer, ServerStage,
};
use crate::port::{
    program_word, AgentLaunch, ConversationEarlier, ConversationLine, ConversationOrigin, ConversationRoad, ConversationStore, EarlierAsk,
    EarlierRead, Env, ExecFailure, StoreError, StoredConversation,
};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_words::js::{js_trim, to_number};

/// Codex's words for a thread another process writes, the -32600 answer's own (0.162.1). TS conversations.ts:16.
pub const WRITER_HELD: &str = "already has an active writer";

/// Codex's words for a thread it holds no such id for: a resume's, a read's, and either's for an id that is no thread
/// id at all (0.162.1). TS conversations.ts:19.
const GONE: [&str; 3] = ["no rollout found", "thread not loaded", "invalid thread id"];

/// TS conversations.ts:21.
pub const LETS_GO: &str = "Codex lets go about a minute after its window closes.";

/// How many pages a list or a read takes at most. TS conversations.ts:26.
pub const LIST_PAGES: usize = 10;

/// What a list prints in place of the server's answers where codex is not on the PATH. TS conversations.ts:41.
pub const NOT_INSTALLED: &str = "wsp-codex-not-installed";

/// TS conversations.ts:25-28.
const PAGE: u64 = 100;
const TURN_PAGE: u64 = 100;
const ITEM_PAGE: u64 = 200;

/// TS conversations.ts:30-32.
const INIT: i64 = 1;
const ASK: i64 = 2;
const OPEN: i64 = 3;

/// Who a script runs codex as. TS conversations.ts:45's `road`.
#[derive(Clone, Default)]
pub struct CodexRoad {
    pub launch: Option<AgentLaunch>,
    pub broker: Option<CodexBroker>,
}

/// A thread as the list shows it, with the rollout path the size is read from. TS conversations.ts:63.
#[derive(Clone, Debug, PartialEq)]
pub struct ListedRow {
    pub row: StoredConversation,
    pub path: Option<String>,
}

/// One page of thread/list. TS conversations.ts:91.
#[derive(Clone, Debug, PartialEq)]
pub struct ListPage {
    pub rows: Vec<ListedRow>,
    pub next: Option<String>,
}

/// A string that is not empty. TS conversations.ts:37.
fn str(value: Option<&Value>) -> Option<&str> {
    value?.as_str().filter(|s| !s.is_empty())
}

/// One script: the server under the environment off its input, its stages in turn, and first the program check where
/// `installed` asks for it. TS conversations.ts:45.
fn script(road: &CodexRoad, stages: &[ServerStage], installed: bool) -> Result<String, String> {
    let check = if installed {
        format!("command -v {} > /dev/null 2>&1 || {{ echo {NOT_INSTALLED}; exit 0; }}\n", program_word("codex", road.launch.as_ref()))
    } else {
        String::new()
    };
    let codex = codex_word(road.launch.as_ref(), road.broker.as_ref())?;
    Ok(format!("cd ~ && {ENV_FROM_INPUT}\n{check}{}", app_server_script(&codex, stages)))
}

fn opening() -> ServerStage {
    ServerStage { lines: vec![app_server_initialize(INIT), app_server_notification("initialized")], answers: 1 }
}

fn ask_line(id: i64, method: &str, params: Value) -> String {
    app_server_request(id, method, params.as_object().cloned().unwrap_or_default())
}

/// TS conversations.ts:54.
pub fn list_command(cwds: &[String], cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    let mut params = json!({"cwd": cwds, "sortKey": "recency_at", "limit": PAGE});
    if let Some(cursor) = cursor {
        params["cursor"] = json!(cursor);
    }
    script(road, &[opening(), ServerStage { lines: vec![ask_line(ASK, "thread/list", params)], answers: 1 }], true)
}

/// The answer to one request, or the server's words for why there is none. TS conversations.ts:84.
fn answer_of<'a>(answers: &'a IndexMap<i64, ServerAnswer>, id: i64, what: &str) -> Result<&'a Map<String, Value>, String> {
    match answers.get(&id) {
        None => Err(format!("codex's app server did not answer {what}")),
        Some(ServerAnswer::Error { message, .. }) => Err(message.clone()),
        Some(ServerAnswer::Result(result)) => Ok(result),
    }
}

/// The objects of a page's `data`, the others skipped. TS conversations.ts:94.
fn data_rows(result: &Map<String, Value>) -> impl Iterator<Item = &Map<String, Value>> {
    result.get("data").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object)
}

/// Who opened a thread, off the source and originator its index row carries. TS conversations.ts:59.
fn origin_of(thread: &Map<String, Value>) -> ConversationOrigin {
    if str(thread.get("originator")) == Some("wsp") {
        ConversationOrigin::Wsp
    } else if matches!(str(thread.get("source")), Some("exec") | Some("appServer")) {
        ConversationOrigin::Script
    } else {
        ConversationOrigin::Terminal
    }
}

/// A thread as the list shows it: the name the person gave it, else its first message. TS conversations.ts:63.
fn listed_row(thread: &Map<String, Value>) -> Option<ListedRow> {
    let id = str(thread.get("id"))?;
    let cwd = str(thread.get("cwd"))?;
    let preview = str(thread.get("preview")).map(|p| js_trim(p).to_string());
    let branch = str(rec(thread.get("gitInfo")).and_then(|g| g.get("branch")));
    let at =
        count(thread.get("recencyAt")).or_else(|| count(thread.get("updatedAt"))).or_else(|| count(thread.get("createdAt"))).unwrap_or(0.0);
    Some(ListedRow {
        row: StoredConversation {
            id: id.to_string(),
            title: str(thread.get("name")).map(String::from).or_else(|| preview.clone()).unwrap_or_else(|| id.to_string()),
            first_prompt: preview,
            branch: branch.map(String::from),
            cwd: cwd.to_string(),
            last_at: at * 1000.0,
            bytes: None,
            origin: origin_of(thread),
        },
        path: str(thread.get("path")).map(String::from),
    })
}

/// TS conversations.ts:91.
pub fn parse_list(stdout: &str) -> Result<ListPage, StoreError> {
    if stdout.split('\n').any(|line| js_trim(line) == NOT_INSTALLED) {
        return Err(StoreError { message: "codex is not installed".into(), lasting: true });
    }
    let answers = answers_of(stdout);
    let result = answer_of(&answers, ASK, "thread/list").map_err(plain_error)?;
    Ok(ListPage { rows: data_rows(result).filter_map(listed_row).collect(), next: str(result.get("nextCursor")).map(String::from) })
}

/// TS conversations.ts:101.
pub fn newest_day_stamp_command(home: &str) -> String {
    format!(
        "d=$(ls -d {}/*/*/* 2>/dev/null | tail -n 1); {}",
        shell_quote(&format!("{home}/sessions")),
        folders_stamp_command(&["\"$d\"".to_string()])
    )
}

/// TS conversations.ts:108.
pub fn sizes_command(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|p| shell_quote(p)).collect();
    format!(
        "for p in {}; do s=$(stat -c %s \"$p\" 2>/dev/null || stat -f %z \"$p\" 2>/dev/null); printf '%s\\n' \"$s\"; done",
        quoted.join(" ")
    )
}

/// TS conversations.ts:112. Always `count` entries.
pub fn parse_sizes(stdout: &str, count: usize) -> Vec<Option<f64>> {
    const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
    let lines: Vec<&str> = stdout.split('\n').collect();
    (0..count)
        .map(|i| {
            let line = js_trim(lines.get(i).copied().unwrap_or(""));
            let n = to_number(line);
            (!line.is_empty() && n.is_finite() && n.fract() == 0.0 && n.abs() <= MAX_SAFE).then_some(n)
        })
        .collect()
}

/// TS conversations.ts:122.
pub fn turns_command(thread_id: &str, probe: bool, cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    let mut params = json!({"threadId": thread_id, "limit": TURN_PAGE, "sortDirection": "desc", "itemsView": "summary"});
    if let Some(cursor) = cursor {
        params["cursor"] = json!(cursor);
    }
    let turns = ask_line(ASK, "thread/turns/list", params);
    if cursor.is_some() {
        return script(road, &[opening(), ServerStage { lines: vec![turns], answers: 1 }], false);
    }
    let open = if probe {
        ask_line(OPEN, "thread/resume", json!({"threadId": thread_id}))
    } else {
        ask_line(OPEN, "thread/read", json!({"threadId": thread_id, "includeTurns": false}))
    };
    script(road, &[opening(), ServerStage { lines: vec![open], answers: 1 }, ServerStage { lines: vec![turns], answers: 1 }], false)
}

/// TS conversations.ts:130.
pub fn items_command(thread_id: &str, cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    let mut params = json!({"threadId": thread_id, "limit": ITEM_PAGE, "sortDirection": "desc"});
    if let Some(cursor) = cursor {
        params["cursor"] = json!(cursor);
    }
    script(road, &[opening(), ServerStage { lines: vec![ask_line(ASK, "thread/items/list", params)], answers: 1 }], false)
}

/// What opening the thread said. TS conversations.ts:136.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    Gone,
    Open,
    Read { cwd: Option<String>, title: Option<String> },
}

/// TS conversations.ts:136.
pub fn parse_opened(stdout: &str) -> Result<Opened, String> {
    let answers = answers_of(stdout);
    let Some(answer) = answers.get(&OPEN) else {
        return Err("codex's app server did not answer opening the thread".into());
    };
    let result = match answer {
        ServerAnswer::Error { message, .. } if message.contains(WRITER_HELD) => return Ok(Opened::Open),
        ServerAnswer::Error { message, .. } if GONE.iter().any(|words| message.contains(words)) => return Ok(Opened::Gone),
        ServerAnswer::Error { message, .. } => return Err(message.clone()),
        ServerAnswer::Result(result) => result,
    };
    let empty = Map::new();
    let thread = rec(result.get("thread")).unwrap_or(&empty);
    let cwd = str(thread.get("cwd")).or(str(result.get("cwd"))).map(String::from);
    let title = str(thread.get("name")).map(String::from).or_else(|| str(thread.get("preview")).map(|p| js_trim(p).to_string()));
    Ok(Opened::Read { cwd, title })
}

/// One turn's id and how many messages its summary holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnCount {
    pub id: String,
    pub messages: u32,
}

/// TS conversations.ts:151.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnsPage {
    pub turns: Vec<TurnCount>,
    pub next: Option<String>,
}

/// TS conversations.ts:151.
pub fn parse_turns(stdout: &str) -> Result<TurnsPage, String> {
    let answers = answers_of(stdout);
    let result = answer_of(&answers, ASK, "thread/turns/list")?;
    let turns = data_rows(result)
        .filter_map(|turn| {
            let id = str(turn.get("id"))?;
            let messages =
                turn.get("items").and_then(Value::as_array).map_or(0, |items| items.iter().filter(|item| is_message(item)).count());
            Some(TurnCount { id: id.to_string(), messages: messages as u32 })
        })
        .collect();
    Ok(TurnsPage { turns, next: str(result.get("nextCursor")).map(String::from) })
}

fn is_message(item: &Value) -> bool {
    matches!(item.get("type").and_then(Value::as_str), Some("userMessage") | Some("agentMessage"))
}

/// One item's row and its turn.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemLine {
    pub turn_id: Option<String>,
    pub line: ConversationLine,
}

/// TS conversations.ts:168.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemsPage {
    pub items: Vec<ItemLine>,
    pub next: Option<String>,
}

/// A call as a thread's earlier messages show it, `{tool, text}`. codex_items' `tool_call_of` is the one handed in.
pub type ToolOf = fn(&Map<String, Value>) -> Option<(String, String)>;

/// TS conversations.ts:184.
fn line_of(item: &Map<String, Value>, tool_of: ToolOf) -> Option<ConversationLine> {
    match str(item.get("type")) {
        Some("userMessage") => {
            let texts: Vec<&str> = item
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| str(part.get("text")))
                .collect();
            let text = js_trim(&texts.join("\n")).to_string();
            (!text.is_empty()).then_some(ConversationLine { who: "person".into(), text, tool: None, at: None })
        }
        Some("agentMessage") => {
            str(item.get("text")).map(|text| ConversationLine { who: "agent".into(), text: text.to_string(), tool: None, at: None })
        }
        _ => tool_of(item).map(|(tool, text)| ConversationLine { who: "tool".into(), text, tool: Some(tool), at: None }),
    }
}

/// TS conversations.ts:168.
pub fn parse_items(stdout: &str, tool_of: ToolOf) -> Result<ItemsPage, String> {
    let answers = answers_of(stdout);
    let result = answer_of(&answers, ASK, "thread/items/list")?;
    let items = data_rows(result)
        .filter_map(|held| {
            let item = rec(held.get("item"))?;
            let mut line = line_of(item, tool_of)?;
            line.at = count(held.get("startedAtMs")).or_else(|| count(held.get("completedAtMs")));
            Some(ItemLine { turn_id: str(held.get("turnId")).map(String::from), line })
        })
        .collect();
    Ok(ItemsPage { items, next: str(result.get("nextCursor")).map(String::from) })
}

/// The newest `last` messages, the calls among them kept, and how many of `total` came before. TS conversations.ts:200.
pub fn newest_lines(items: &[ConversationLine], total: u32, last: u32) -> ConversationEarlier {
    let mut kept: Vec<ConversationLine> = Vec::new();
    let mut messages: u32 = 0;
    for line in items.iter().rev() {
        if line.who != "tool" {
            if messages == last {
                break;
            }
            messages += 1;
        }
        kept.push(line.clone());
    }
    kept.reverse();
    let calls_before_the_first_message = kept.iter().take_while(|line| line.who == "tool").count();
    kept.drain(..calls_before_the_first_message);
    ConversationEarlier { cwd: None, title: None, lines: kept, earlier: total.saturating_sub(messages) }
}

/// The threads Codex kept on a computer. TS adapter.ts:1333.
pub struct CodexConversations {
    pub home: String,
    pub road: CodexRoad,
    /// The environment every question carries off its input: the turn's own where brokered, else build_env's. TS
    /// adapter.ts:1331.
    pub question_env: Env,
}

fn plain_error(message: String) -> StoreError {
    StoreError { message, lasting: false }
}

fn exec_error(error: ExecFailure) -> StoreError {
    StoreError { message: error.message, lasting: false }
}

/// TS adapter.ts:1336-1351.
async fn list_rows(codex: &CodexRoad, env: &Env, cwds: &[String], road: &ConversationRoad) -> Result<Vec<StoredConversation>, StoreError> {
    if cwds.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows: Vec<ListedRow> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..LIST_PAGES {
        let command = list_command(cwds, cursor.as_deref(), codex).map_err(plain_error)?;
        let page = parse_list(&road.exec.exec(&command, Some(env)).await.map_err(exec_error)?)?;
        rows.extend(page.rows);
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    let paths: Vec<String> = rows.iter().filter_map(|row| row.path.clone()).collect();
    let mut size_of: HashMap<String, Option<f64>> = HashMap::new();
    if !paths.is_empty() {
        let stdout = road.exec.exec(&sizes_command(&paths), None).await.unwrap_or_default();
        for (path, size) in paths.iter().zip(parse_sizes(&stdout, paths.len())) {
            size_of.insert(path.clone(), size);
        }
    }
    Ok(rows
        .into_iter()
        .map(|listed| {
            let bytes = listed.path.as_ref().and_then(|path| size_of.get(path).copied().flatten());
            StoredConversation { bytes, ..listed.row }
        })
        .collect())
}

/// TS adapter.ts:1353-1387.
async fn earlier_read(
    codex: &CodexRoad,
    env: &Env,
    road: &ConversationRoad,
    id: &str,
    ask: &EarlierAsk,
) -> Result<EarlierRead, StoreError> {
    let thread_id = slug("threadId", id).map_err(plain_error)?;
    let first =
        road.exec.exec(&turns_command(&thread_id, ask.probe, None, codex).map_err(plain_error)?, Some(env)).await.map_err(exec_error)?;
    let (cwd, title) = match parse_opened(&first).map_err(plain_error)? {
        Opened::Gone => return Ok(EarlierRead::Gone),
        Opened::Open => return Ok(EarlierRead::Open),
        Opened::Read { cwd, title } => (cwd, title),
    };
    let mut page = parse_turns(&first).map_err(plain_error)?;
    let mut turns = page.turns.clone();
    let mut read = 1;
    while ask.count && page.next.is_some() && read < LIST_PAGES {
        let stdout = road
            .exec
            .exec(&turns_command(&thread_id, false, page.next.as_deref(), codex).map_err(plain_error)?, Some(env))
            .await
            .map_err(exec_error)?;
        page = parse_turns(&stdout).map_err(plain_error)?;
        turns.extend(page.turns.iter().cloned());
        read += 1;
    }
    let total: u32 = turns.iter().map(|turn| turn.messages).sum();
    let mut kept: HashSet<String> = HashSet::new();
    let mut held: u32 = 0;
    for turn in &turns {
        if held >= ask.last {
            break;
        }
        kept.insert(turn.id.clone());
        held += turn.messages;
    }
    let mut read_lines: Vec<ConversationLine> = Vec::new();
    let mut cursor: Option<String> = None;
    'reading: for _ in 0..LIST_PAGES {
        let stdout = road
            .exec
            .exec(&items_command(&thread_id, cursor.as_deref(), codex).map_err(plain_error)?, Some(env))
            .await
            .map_err(exec_error)?;
        let items = parse_items(&stdout, tool_call_of).map_err(plain_error)?;
        for item in items.items {
            if item.turn_id.as_ref().is_some_and(|turn| !kept.contains(turn)) {
                break 'reading;
            }
            read_lines.push(item.line);
        }
        match items.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    read_lines.reverse();
    let newest = newest_lines(&read_lines, total, ask.last);
    Ok(EarlierRead::Read(ConversationEarlier { cwd, title, lines: newest.lines, earlier: if ask.count { newest.earlier } else { 0 } }))
}

impl ConversationStore for CodexConversations {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        let codex = self.road.clone();
        let env = self.question_env.clone();
        async move { list_rows(&codex, &env, &cwds, &road).await }.boxed_local()
    }
    fn stamp(&self, _cwds: Vec<String>, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Result<String, StoreError>>> {
        let command = newest_day_stamp_command(&self.home);
        Some(async move { road.exec.exec(&command, None).await.map_err(exec_error) }.boxed_local())
    }
    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        let codex = self.road.clone();
        let env = self.question_env.clone();
        async move { earlier_read(&codex, &env, &road, &id, &ask).await }.boxed_local()
    }
    fn lets_go(&self) -> Option<String> {
        Some(LETS_GO.to_string())
    }
}

/// The store as the adapter hands it out. TS adapter.ts:1331-1333.
pub fn codex_conversations(home: &str, road: CodexRoad, question_env: Env) -> Arc<CodexConversations> {
    Arc::new(CodexConversations { home: home.to_string(), road, question_env })
}
