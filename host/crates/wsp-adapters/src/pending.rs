// SPDX-License-Identifier: AGPL-3.0-only
//! PENDING: what this crate reads from packages/protocol that wave 1 (`wsp-wire`, `wsp-words`, `wsp-catalog`, on the
//! `rust/2445-*` branches read at a3b870a6e) does not carry yet. Each item names the TS it stands for and the wave-1
//! crate it belongs in. When that crate gains it, the item goes and its users import the wave-1 one. Bodies are the
//! TS's own, line for line, and are not this crate's to design: the ported adapter tests read their exact words.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::port::{PermissionOption, TurnResult};
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::PatchHunk;

/// PENDING(wsp-wire): packages/protocol/src/views/harness.ts:161, a TS interface the zod generator skipped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSpec {
    pub command: String,
    pub args: Vec<String>,
    /// `Some(true)` only; never put on a launch line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_slate: Option<bool>,
}

/// PENDING(wsp-words): words/turn.ts:172. Why a model call is being made again, in plain words, off the harness's
/// error category and HTTP status.
pub fn anthropic_retry_why(error: Option<&str>, status: Option<f64>) -> String {
    todo!()
}

/// PENDING(wsp-words): usage.ts:342. The model less a trailing `[...]` context suffix.
pub fn base_model(model: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:123.
pub const QUESTION_TOOL: &str = "AskUserQuestion";

/// PENDING(wsp-words): words/thread.ts:461.
pub fn aside_wall_line(ms: u64) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:469.
pub const ASIDE_TOOL_LINE: &str = "the answer reached for a tool, and a side question runs none; ask it again, or send it as a message for the thread to work on";

/// PENDING(wsp-words): words/thread.ts:256. The last line of a held reply whose process ended with work still running.
pub fn background_tasks_line(running: u32) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:270. One line per background task that finished after a held reply.
pub fn task_finished_line(description: &str, status: &str, ms_after_reply: f64) -> String {
    todo!()
}

/// PENDING(wsp-words): project-path.ts:65.
pub fn claude_memory_dir(state_home: &str, key: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): project-path.ts:60. Every byte outside `[A-Za-z0-9]` as `-`, per UTF-16 unit as the TS does.
pub fn claude_project_key(path: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:54.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HarnessEnded {
    pub signal: Option<String>,
    pub reached: Option<bool>,
}

/// PENDING(wsp-words): words/thread.ts:40. The turn's error when the process ended before any result.
pub fn harness_exit_line(bin: &str, exit_code: Option<i32>, path: Option<&str>, ended: &HarnessEnded) -> String {
    todo!()
}

/// wsp's half of a refusal: the road out where the caller knows one, and the cause it is classed by. TS
/// `{ road?: string; cause?: TurnRefusal }` in words/turn.ts:221.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refusal {
    pub road: Option<String>,
    pub cause: Option<TurnRefusal>,
}

/// PENDING(wsp-words): words/turn.ts:221.
pub fn refused_turn(result: TurnResult, refusal: Option<&Refusal>) -> TurnResult {
    todo!()
}

/// PENDING(wsp-words): words/turn.ts:42.
pub const SUBAGENT_ASKED_CHARS: usize = 280;

/// PENDING(wsp-words): words/turn.ts:65. The start of what a subagent was asked, cut to SUBAGENT_ASKED_CHARS.
pub fn subagent_asked(prompt: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/turn.ts:34.
pub const NO_REPORT_LINE: &str = "Finished without handing back a report.";

/// PENDING(wsp-words): words/turn.ts:268.
pub const LOST_SESSION_NOTE: &str = "the agent's session for this thread was gone from the machine, so this turn started a new one and handed it the thread so far";

/// PENDING(wsp-words): words/turn.ts, `lostSessionPrompt`. The prompt a new session is handed the thread so far with.
pub fn lost_session_prompt(seed: &str, prompt: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/turn.ts:353.
pub fn title_prompt(opening: &str, reply: Option<&str>) -> String {
    todo!()
}

/// PENDING(wsp-words): words/turn.ts:373. A title out of an answer, or `None` where the answer is not one.
pub fn generated_title(answer: &str) -> Option<String> {
    todo!()
}

/// PENDING(wsp-words): words/turn.ts:75. Test-only reader in the TS suite.
pub fn notify_tail(result: &TurnResult) -> Option<String> {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:379. Test-only reader in the TS suite.
pub fn internal_tool_result(text: &str) -> bool {
    todo!()
}

/// PENDING(wsp-words): words/units.ts:380, the "short" style.
pub fn fmt_duration(ms: f64) -> String {
    todo!()
}

/// PENDING(wsp-words): shell-quote.ts:10. `cd <cwd quoted> && <command>`, or `cd ~ && ` with no cwd.
pub fn in_folder(cwd: Option<&str>, command: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): shell-quote.ts:23. Each word bare where sh reads it as itself, quoted otherwise.
pub fn shell_line(words: &[String]) -> String {
    todo!()
}

/// PENDING(wsp-words): wsp-tools.ts:30. Whether a launch's servers give its thread a slate.
pub fn launch_has_slate(servers: Option<&IndexMap<String, McpServerSpec>>) -> bool {
    todo!()
}

/// PENDING(wsp-words): tool-result.ts:7. Every line of `content` as one hunk of `sign` lines.
pub fn whole_file_hunk(content: &str, sign: char) -> PatchHunk {
    todo!()
}

/// PENDING(wsp-words): conversations.ts:103.
pub fn folders_stamp_command(folders: &[String]) -> String {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:129. One choice of a question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskedOption {
    pub id: String,
    pub label: String,
    pub description: String,
}

/// PENDING(wsp-words): words/tools.ts:135.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskedQuestion {
    pub key: String,
    pub index: usize,
    pub header: String,
    pub question: String,
    pub options: Vec<AskedOption>,
    pub multi_select: bool,
}

/// PENDING(wsp-words): words/tools.ts:167.
pub fn asked_questions(tool_name: &str, input: &str) -> Option<Vec<AskedQuestion>> {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:189.
pub fn question_options(tool_name: &str, input: &str) -> Vec<PermissionOption> {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:196. `q<question>:other:<encodeURIComponent(words)>`.
pub fn other_option_id(question: usize, words: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:222. The ids joined by `|`.
pub fn picked_option_id(ids: &[String]) -> String {
    todo!()
}

/// PENDING(wsp-words): workspace-state.ts:600 (`signInRefusalLine`) for the two kinds this crate's tests name: the
/// person's own computer (`local`), else a cloud's machine (CLOUD_SIGN_IN_ROAD).
pub fn sign_in_refusal_line(local: bool) -> String {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:212.
pub fn picked_options(options: &[PermissionOption], picked: &str) -> Option<Vec<PermissionOption>> {
    todo!()
}

/// PENDING(wsp-words): words/tools.ts:227. The call's input with the answers written in, keys in the input's order
/// and `answers` last.
pub fn question_answer_input(tool_name: &str, input: &str, picked: &[String]) -> Option<Map<String, Value>> {
    todo!()
}

// Codex, OpenCode and Cursor (the others writer, #2447). Same rule as above: each goes once its wave-1 crate has it.

/// PENDING(wsp-words): shell-quote.ts:4. One POSIX sh word: `'...'`, each `'` inside written `'\''`.
pub fn shell_quote(value: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): tool-result.ts:20. The hunks of a unified diff: one trailing `\n` dropped, split on `\n`, a
/// line matching `^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@` opens a hunk (a missing count reads 1), every other
/// line joins the last hunk, and lines before the first head are dropped.
pub fn unified_hunks(diff: &str) -> Vec<PatchHunk> {
    todo!()
}

/// PENDING(wsp-words): conversations.ts:164, through refusalLine.
pub fn conversation_written_elsewhere_line(agent: &str, lets_go: Option<&str>) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:622.
pub fn codex_not_signed_in_line(login: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:627.
pub fn codex_missing_env_line(name: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:651, `fmtDuration(ms, "clock")` inside.
pub fn codex_reconnect_line(elapsed_ms: f64) -> String {
    todo!()
}

/// PENDING(wsp-words): words/places.ts:323.
pub fn codex_key_refused_line(key_env: &str, reason: &str, login: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): words/thread.ts:504.
pub const CODEX_LEGACY_HISTORY: &str = "a Codex older than 0.151.0 made this thread and keeps its history in a form thread/revert cannot cut";

/// PENDING(wsp-words): words/thread.ts:508.
pub const CODEX_FEWER_TURNS: &str = "codex lists fewer turns in this thread than the rewind would cut, so it cut nothing";

/// PENDING(wsp-words): words/thread.ts:511.
pub fn codex_no_turn_line(turn_id: &str) -> String {
    todo!()
}

/// PENDING(wsp-words): usage.ts:49. Up to 300 minutes the session, up to 10080 the week, else the month.
pub fn limit_kind_of_minutes(minutes: f64) -> wsp_wire::types::usage::LimitKind {
    todo!()
}

/// PENDING(wsp-wire): placeholders.ts:18, a TS interface the zod generator skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexClaims {
    pub account_id: String,
    pub plan_type: Option<String>,
    pub user_id: Option<String>,
    pub fedramp: Option<bool>,
}

/// PENDING(wsp-catalog): codex-app-server.ts:17. How long the probe's reader waits for one more line, in seconds.
pub const LINE_WAIT_S: u64 = 10;

/// PENDING(wsp-catalog): codex-app-server.ts:19. `{"jsonrpc":"2.0","id":..,"method":..,"params":..}`.
pub fn app_server_request(id: i64, method: &str, params: Map<String, Value>) -> String {
    todo!()
}

/// PENDING(wsp-catalog): codex-app-server.ts:21. `{"jsonrpc":"2.0","method":..,"params":{}}`.
pub fn app_server_notification(method: &str) -> String {
    todo!()
}

/// PENDING(wsp-catalog): codex-app-server.ts:23. `initialize` with `clientInfo {name: "wsp", version: "0"}`.
pub fn app_server_initialize(id: i64) -> String {
    todo!()
}

/// PENDING(wsp-catalog): codex-app-server.ts:26.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerStage {
    pub lines: Vec<String>,
    pub answers: u32,
}

/// PENDING(wsp-catalog): codex-app-server.ts:32. The server under `codex` fed stage by stage through two fifos.
pub fn app_server_script(codex: &str, stages: &[ServerStage]) -> String {
    todo!()
}

/// PENDING(wsp-catalog): codex-app-server.ts:62.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerAnswer {
    Result(Map<String, Value>),
    Error { code: Option<f64>, message: String, data: Option<Map<String, Value>> },
}

/// PENDING(wsp-catalog): codex-app-server.ts:69. Each answer by its numeric id; a result is kept only when it is an object.
pub fn answers_of(text: &str) -> IndexMap<i64, ServerAnswer> {
    todo!()
}

/// PENDING(wsp-catalog): codex-app-server.ts:94.
pub fn result_of(answers: &IndexMap<i64, ServerAnswer>, id: i64) -> Option<&Map<String, Value>> {
    todo!()
}

/// PENDING(wsp-catalog): agents/codex.ts:25, the npm pin of codex's install road.
pub const CODEX_PINNED_VERSION: &str = "0.155.1";
