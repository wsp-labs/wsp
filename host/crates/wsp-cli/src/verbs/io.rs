// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/io.ts` at 65f6b89c7: the roads that follow a pushed stream (exec, export, import), the
//! plan's lines, the one confirmation, the tool door's answers and output shapes, and the tool inputs several verbs
//! share. Spec: `spec/io.md`.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::oneshot;

use super::workspaces_help::HostRow;
use crate::init_layout::colour_depth;
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::git_hosts::THREAD_AGENTS;
use wsp_wire::types::attachments::{FILES_MAX, FILE_MAX_WORDS, IMAGE_MAX_WORDS, IMAGE_TYPE_WORDS};
use wsp_wire::types::check::{js_trim, parse};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::project_import::ProjectImportRequest;
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::project_bundle::{HostFolderListing, ProjectExportResult, ProjectImportResult, ProjectPlan, TerminalScheme};
use wsp_wire::views::session::{ThreadCapWait, ThreadView};
use wsp_wire::views::session_events::TurnStatus;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView};
use wsp_wire::wire::capabilities::InitSetup;
use wsp_wire::wire::runtime::{ExecEvent, SessionStartOutcome};
use wsp_words::exit::{json_line, usage_refusal, HostError, Kind};
use wsp_words::js::to_number;
use wsp_words::place_state::cap_wait_line;
use wsp_words::project_import::{can_travel, default_agents, default_consent, secret_offer};
use wsp_words::words::base::plural;
use wsp_words::words::places::secret_signals_line;
use wsp_words::words::thread::{folder_level_line, still_working_line, unknown_agent_line, AFTER_CUT_LINE, NOTIFY_CALLER};
use wsp_words::words::turn::{notify_line, notify_tail, wait_timed_out_line};
use wsp_words::words::units::fmt_bytes;

use super::client::{
    absolute_path, flag_set, opened_thread_line, other_version, place_named, table, thread_id_of, until_settled, CallToolResult, CliIo,
    FlagRow, FlagTable, FlagValue, Flags, Frame, HostClient, InputKind, OpenedFn, PushedFrames, TextContent, ToolInput, VerbContext,
};
use super::turns_help::{turn_refusal, turn_view, Ended, Turn, Waited};

// ---------------------------------------------------------------------------------------------------------------
// The roads that follow a pushed stream (io.ts:57-143).
// ---------------------------------------------------------------------------------------------------------------

/// `ExecExit` (io.ts:57): the `exec.exit` frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecExit {
    pub exec_id: String,
    /// `null` where the command had no exit code (a signal, a machine gone).
    pub exit_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

/// `ExecRun` (io.ts:61-64).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecRun {
    pub exit: ExecExit,
    /// The folder the host answered with; None only where it named none.
    pub ran_in: Option<String>,
}

/// What `exec_on` hands each frame of its command to.
pub type ExecSink = Arc<dyn Fn(&ExecEvent) + Send + Sync>;

/// `execOn(client, workspaceId, argv, cwd, on)` (io.ts:68-86): frames held from before `workspaces.exec` is sent
/// (`{workspaceId, argv, cwd?}`), then every `exec.output` and `exec.exit` whose `execId` is the reply's, in arrival
/// order, to `on`; the first exit ends it. A socket gone first fails it with `close_words()`. The held frames stop
/// on every road out.
pub async fn exec_on(
    client: &dyn HostClient,
    workspace_id: &str,
    argv: &[String],
    cwd: Option<&str>,
    on: ExecSink,
) -> Result<ExecRun, HostError> {
    let pushed = PushedFrames::new(client);
    let run = exec_follow(client, &pushed, workspace_id, argv, cwd, on).await;
    pushed.stop();
    run
}

async fn exec_follow(
    client: &dyn HostClient,
    pushed: &PushedFrames,
    workspace_id: &str,
    argv: &[String],
    cwd: Option<&str>,
    on: ExecSink,
) -> Result<ExecRun, HostError> {
    let mut params = Frame::new();
    params.insert("workspaceId".to_owned(), json!(workspace_id));
    params.insert("argv".to_owned(), json!(argv));
    if let Some(cwd) = cwd {
        params.insert("cwd".to_owned(), json!(cwd));
    }
    let reply = client.request("workspaces.exec", params).await?;
    let exec_id: String = field_of(&reply, "workspaces.exec", "execId")?;
    let ran_in = reply.get("cwd").and_then(Value::as_str).map(str::to_owned);
    let (done, exited) = oneshot::channel::<ExecExit>();
    let done = Mutex::new(Some(done));
    let wanted = exec_id.clone();
    pushed.follow(
        Arc::new(move |frame: &Frame| {
            matches!(frame.get("type").and_then(Value::as_str), Some("exec.output" | "exec.exit"))
                && frame.get("execId").and_then(Value::as_str) == Some(wanted.as_str())
        }),
        Arc::new(move |frame: &Frame| {
            let Ok(event) = serde_json::from_value::<ExecEvent>(Value::Object(frame.clone())) else { return };
            let mut slot = done.lock().unwrap();
            if slot.is_none() {
                return;
            }
            on(&event);
            if let ExecEvent::ExecExit { exec_id, exit_code, error } = event {
                if let Some(done) = slot.take() {
                    let _ = done.send(ExecExit { exec_id, exit_code: exit_code.map(|code| code.0 .0 as i64), error });
                }
            }
        }),
    );
    let exit = until_settled(client, Box::pin(async move { exited.await.map_err(|_| HostError::new(client.close_words())) })).await?;
    Ok(ExecRun { exit, ran_in })
}

/// `ExportRequest` (io.ts:90-95).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportRequest {
    pub source: String,
    pub dest: String,
    pub replace: Option<bool>,
    pub agents: Option<Vec<String>>,
}

/// One `project.export` or `project.import` frame as the runtime pushed it.
pub type StageSink = Arc<dyn Fn(&Value) + Send + Sync>;

/// `exportProject(client, workspaceId, req, on)` (io.ts:99-121): frames held, `events.subscribe`, then every
/// `project.export` frame whose `workspaceId` and `dest` are this export's to `on`; the request
/// `{workspaceId, source, dest, replace?, agents?}` raced against the socket; answers `exported`.
pub async fn export_project(
    client: &dyn HostClient,
    workspace_id: &str,
    req: &ExportRequest,
    on: StageSink,
) -> Result<ProjectExportResult, HostError> {
    let pushed = PushedFrames::new(client);
    let exported = export_follow(client, &pushed, workspace_id, req, on).await;
    pushed.stop();
    exported
}

async fn export_follow(
    client: &dyn HostClient,
    pushed: &PushedFrames,
    workspace_id: &str,
    req: &ExportRequest,
    on: StageSink,
) -> Result<ProjectExportResult, HostError> {
    client.events().await?;
    let (wanted_workspace, wanted_dest) = (workspace_id.to_owned(), req.dest.clone());
    pushed.follow(
        Arc::new(move |frame: &Frame| {
            frame.get("type").and_then(Value::as_str) == Some("project.export")
                && frame.get("workspaceId").and_then(Value::as_str) == Some(wanted_workspace.as_str())
                && frame.get("dest").and_then(Value::as_str) == Some(wanted_dest.as_str())
        }),
        Arc::new(move |frame: &Frame| on(&Value::Object(frame.clone()))),
    );
    let mut params = Frame::new();
    params.insert("workspaceId".to_owned(), json!(workspace_id));
    params.insert("source".to_owned(), json!(req.source));
    params.insert("dest".to_owned(), json!(req.dest));
    if let Some(replace) = req.replace {
        params.insert("replace".to_owned(), json!(replace));
    }
    if let Some(agents) = &req.agents {
        params.insert("agents".to_owned(), json!(agents));
    }
    let reply = until_settled(client, client.request("project.export", params)).await?;
    field_of(&reply, "project.export", "exported")
}

/// `planProject(client, source)` (io.ts:124-126): `project.plan {source}`'s `plan`.
pub async fn plan_project(client: &dyn HostClient, source: &str) -> Result<ProjectPlan, HostError> {
    let reply = client.request("project.plan", pick_frame([("source", json!(source))])).await?;
    field_of(&reply, "project.plan", "plan")
}

/// `importProject(client, workspaceId, req, on)` (io.ts:130-143): as export, matched on `workspaceId`, `source` and
/// `dest`; the request is `{workspaceId, ...req}`.
pub async fn import_project(
    client: &dyn HostClient,
    workspace_id: &str,
    req: &ProjectImportRequest,
    on: StageSink,
) -> Result<ProjectImportResult, HostError> {
    let pushed = PushedFrames::new(client);
    let imported = import_follow(client, &pushed, workspace_id, req, on).await;
    pushed.stop();
    imported
}

async fn import_follow(
    client: &dyn HostClient,
    pushed: &PushedFrames,
    workspace_id: &str,
    req: &ProjectImportRequest,
    on: StageSink,
) -> Result<ProjectImportResult, HostError> {
    client.events().await?;
    let (wanted_workspace, wanted_source, wanted_dest) = (workspace_id.to_owned(), req.source.clone(), req.dest.clone());
    pushed.follow(
        Arc::new(move |frame: &Frame| {
            frame.get("type").and_then(Value::as_str) == Some("project.import")
                && frame.get("workspaceId").and_then(Value::as_str) == Some(wanted_workspace.as_str())
                && frame.get("source").and_then(Value::as_str) == Some(wanted_source.as_str())
                && frame.get("dest").and_then(Value::as_str) == Some(wanted_dest.as_str())
        }),
        Arc::new(move |frame: &Frame| on(&Value::Object(frame.clone()))),
    );
    let mut params = pick_frame([("workspaceId", json!(workspace_id))]);
    if let Value::Object(request) = serde_json::to_value(req).map_err(|e| HostError::new(e.to_string()))? {
        params.extend(request);
    }
    let reply = until_settled(client, client.request("project.import", params)).await?;
    field_of(&reply, "project.import", "imported")
}

// ---------------------------------------------------------------------------------------------------------------
// The plan (io.ts:147-197).
// ---------------------------------------------------------------------------------------------------------------

/// `secretsChosen(plan, keep, cut)` (io.ts:147-156): every keep and cut path must be a secret the plan lists, keeps
/// first then cuts, the first stranger refused; then the plan's default ticks, plus keep, less cut.
pub fn secrets_chosen(plan: &ProjectPlan, keep: &[String], cut: &[String]) -> Result<BTreeSet<String>, HostError> {
    let listed: IndexSet<String> = plan.secrets.iter().map(|secret| secret.path.clone()).collect();
    for path in keep.iter().chain(cut) {
        if !listed.contains(path) {
            let lists = if listed.is_empty() {
                ", which lists none".to_owned()
            } else {
                format!("; the plan lists {}", listed.iter().map(String::as_str).collect::<Vec<_>>().join(", "))
            };
            return Err(usage_refusal(
                &format!("{path} is not a secret-shaped file in the plan{lists}."),
                "Name one the plan lists, or drop the flag.",
            ));
        }
    }
    let mut ticked = default_consent(&plan.secrets);
    for path in keep {
        ticked.insert(path.clone());
    }
    for path in cut {
        ticked.shift_remove(path);
    }
    Ok(ticked.into_iter().collect())
}

/// `agentsChosen(plan, named)` (io.ts:159-166): None is the plan's default; each named agent must travel.
pub fn agents_chosen(plan: &ProjectPlan, named: Option<&[String]>) -> Result<BTreeSet<String>, HostError> {
    let Some(named) = named else {
        return Ok(default_agents(&plan.agents).into_iter().collect());
    };
    let travelling: Vec<&str> = plan.agents.iter().filter(|agent| can_travel(agent)).map(|agent| agent.agent.as_str()).collect();
    for id in named {
        if !travelling.contains(&id.as_str()) {
            let lists = if travelling.is_empty() { String::new() } else { format!("; the plan lists {}", travelling.join(", ")) };
            return Err(usage_refusal(
                &format!("{id} has no sessions for this folder{lists}."),
                "Name one the plan lists, or drop the flag and let every agent with sessions come.",
            ));
        }
    }
    Ok(named.iter().cloned().collect())
}

/// `planLines(plan, ticked, agents, workspace)` (io.ts:171-185).
pub fn plan_lines(
    plan: &ProjectPlan,
    ticked: &BTreeSet<String>,
    agents: &BTreeSet<String>,
    workspace: Option<&WorkspaceView>,
) -> Vec<String> {
    let _ = workspace;
    let files = plural(plan.files.0, "file");
    let mut rows: Vec<Vec<String>> = vec![
        vec!["Repository".to_owned(), if plan.repo { "git, .git travels whole".to_owned() } else { "none".to_owned() }],
        vec!["Files".to_owned(), format!("{files}, {}", fmt_bytes(plan.bytes.0))],
        vec!["Caches left behind".to_owned(), if plan.excluded.is_empty() { "none".to_owned() } else { plan.excluded.join(", ") }],
        vec!["Not carried".to_owned(), if plan.skipped.is_empty() { "none".to_owned() } else { plural(plan.skipped.len() as f64, "path") }],
    ];
    rows.extend(plan.skipped.iter().map(|skipped| vec![format!("  {}", skipped.path), skipped.note.clone()]));
    rows.push(vec!["Lands at".to_owned(), plan.source.clone()]);
    rows.push(vec![
        "Secret-shaped".to_owned(),
        if plan.secrets.is_empty() { "none".to_owned() } else { plural(plan.secrets.len() as f64, "file") },
    ]);
    rows.extend(plan.secrets.iter().map(|secret| {
        vec![format!("  {}", secret.path), secret_signals_line(secret), secret_offer(secret, ticked.contains(&secret.path)).full]
    }));
    rows.push(vec![
        "Agents".to_owned(),
        if plan.agents.is_empty() {
            "none with sessions for the folder".to_owned()
        } else {
            format!("{} with sessions for the folder", plural(plan.agents.len() as f64, "agent"))
        },
    ]);
    rows.extend(plan.agents.iter().map(|agent| {
        vec![
            format!("  {}", agent.name),
            agent.error.clone().unwrap_or_else(|| plural(agent.sessions.0, "session")),
            if agents.contains(&agent.agent) { "sessions travel".to_owned() } else { "stays".to_owned() },
        ]
    }));
    table(&rows)
}

/// `PLAN_ONLY` (io.ts:188).
pub const PLAN_ONLY: &str =
    "nothing imported; run again with --yes to take these defaults, or --keep <path> and --cut <path> per secret-shaped row";

/// `IMPORT_NOW` (io.ts:191).
pub const IMPORT_NOW: &str = "Import now? y/N";

/// `withReplaceHint(e)` (io.ts:194-197): an error of kind `exists` gains `\nRun again with --replace to overwrite
/// it.` and keeps the kind; anything else as it came.
pub fn with_replace_hint(e: HostError) -> HostError {
    if e.kind != Some(Kind::Exists) {
        return e;
    }
    HostError::with_kind(format!("{}\nRun again with --replace to overwrite it.", e.message), Kind::Exists)
}

/// `agentsFlag(value)` (io.ts:200-208): None for None; split on `,`, each trimmed, empties dropped; none left is
/// refused; the first id the catalog does not know is refused naming the catalog's ids.
pub fn agents_flag(value: Option<&str>) -> Result<Option<Vec<String>>, HostError> {
    let Some(value) = value else { return Ok(None) };
    let ids: Vec<String> = value.split(',').map(|id| js_trim(id).to_owned()).filter(|id| !id.is_empty()).collect();
    if ids.is_empty() {
        return Err(usage_refusal("--agents was given no agent.", "Write it as --agents claude,codex, or drop the flag."));
    }
    let known: Vec<String> = catalog_agents().iter().map(|agent| agent.id.clone()).collect();
    if let Some(unknown) = ids.iter().find(|id| !known.contains(id)) {
        return Err(usage_refusal(&unknown_agent_line(unknown, &known), "Name one of those, or drop the flag."));
    }
    Ok(Some(ids))
}

// ---------------------------------------------------------------------------------------------------------------
// The one confirmation (io.ts:212-223) and the quiet tool door (io.ts:226-229).
// ---------------------------------------------------------------------------------------------------------------

/// `confirmed(ctx, question, name)` (io.ts:212-214): `confirmed_at` with `--yes` off the line.
pub async fn confirmed(ctx: &VerbContext, question: &str, name: &str) -> Result<bool, HostError> {
    confirmed_at(ctx.io.as_ref(), flag_set(&ctx.flags, "yes"), question, name).await
}

/// `confirmedAt(io, yes, question, name)` (io.ts:217-223): yes is true; off a terminal a usage refusal
/// `<question's first line> There is no terminal to answer on.` with `Pass --yes to say yes.`, nothing asked; at a
/// terminal `ask`, "yes" is true, anything else prints `<name> kept` on stderr and is false.
pub async fn confirmed_at(io: &dyn CliIo, yes: bool, question: &str, name: &str) -> Result<bool, HostError> {
    if yes {
        return Ok(true);
    }
    if !io.is_tty() {
        let first = question.split('\n').next().unwrap_or_default();
        return Err(usage_refusal(&format!("{first} There is no terminal to answer on."), "Pass --yes to say yes."));
    }
    if io.ask(question).await? == "yes" {
        return Ok(true);
    }
    io.error(&format!("{name} kept"));
    Ok(false)
}

/// `QUIET_LINE` (io.ts:228): the waking line dropped on the tool door.
pub fn quiet_line(line: &str) {
    let _ = line;
}

// ---------------------------------------------------------------------------------------------------------------
// The tool door's words and output shapes (io.ts:231-326).
// ---------------------------------------------------------------------------------------------------------------

/// `Created` (io.ts:231).
#[derive(Clone, Debug, PartialEq)]
pub struct Created {
    pub workspace: WorkspaceOut,
    /// The workspace as the host wrote it: what the answer prints, since the TS passes the host's object on.
    pub shown: Option<Value>,
    pub notice: Option<String>,
}

impl Serialize for Created {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let workspace = match &self.shown {
            Some(shown) => shown.clone(),
            None => serde_json::to_value(&self.workspace).map_err(S::Error::custom)?,
        };
        let mut written = serde_json::Map::new();
        written.insert("workspace".to_owned(), workspace);
        if let Some(notice) = &self.notice {
            written.insert("notice".to_owned(), Value::from(notice.as_str()));
        }
        Value::Object(written).serialize(serializer)
    }
}

/// `SEND_MEETS` (io.ts:236), built from `SessionStartOutcome`'s words and `still_working_line()`.
pub fn send_meets() -> String {
    let word =
        |outcome: SessionStartOutcome| serde_json::to_value(outcome).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    let (started, steered, queued) =
        (word(SessionStartOutcome::Started), word(SessionStartOutcome::Steered), word(SessionStartOutcome::Queued));
    let still = still_working_line(None);
    format!(
        "On a thread whose turn is not running the message starts a new turn (outcome `{started}`); when the turn is still running the message joins it (outcome `{steered}`) or waits for it and then runs (outcome `{queued}`), and the reply is that turn's. When the thread's turn has replied but its agent process is still running, the message waits for that process and runs as the thread's next turn (outcome `{queued}`), which the runtime says as `{still}`. A send is never refused for meeting a turn, and two sends keep the order they arrived in."
    )
}

/// `CAP_MEETS` (io.ts:239).
pub fn cap_meets() -> String {
    "A start on a computer already running as many threads as its threads at once waits for a slot and starts on its own when one frees (outcome `held`), with capped beside it, the computer and what runs there against its number, which the thread's row in threads carries while it waits; a thread that follows a run or a send to its end, not detached, lends that thread's next turn its own slot, one turn at a time, a turn already held ahead of the message included, and every other start, a child started detached included, waits for a slot of its own".to_owned()
}

/// `TurnOut` (io.ts:243-252), keys in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnOut {
    pub thread_id: String,
    pub workspace_id: String,
    pub harness: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub text: Option<String>,
    pub outcome: SessionStartOutcome,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub capped: Option<ThreadCapWait>,
    /// `z.literal(true)`: Some(true) or absent.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub after_cut: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub notice: Option<String>,
}

/// `RunOut` (io.ts:254): a turn's answer and, on a sub-thread's start, `sub` after every TurnOut key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunOut {
    #[serde(flatten)]
    pub turn: TurnOut,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sub: Option<SubOut>,
}

/// `ThreadEndOut` (io.ts:257-264).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEndOut {
    pub thread_id: String,
    pub status: TurnStatus,
    #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "js_number_opt")]
    pub duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "js_number_opt")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<SubResult>,
}

/// A JS number as the wire writes one: a whole number without its point.
fn js_number_opt<S: serde::Serializer>(value: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(n) => JsNumber(*n).serialize(s),
        None => s.serialize_none(),
    }
}

/// `WaitOut` (io.ts:265-268).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitOut {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub finished: Option<ThreadEndOut>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub timed_out: Option<bool>,
}

/// `ThreadRowOut` (io.ts:269): the thread's row, then these four.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRowOut {
    #[serde(flatten)]
    pub thread: ThreadView,
    pub project_name: String,
    pub folder: String,
    pub branch: String,
    pub computer_name: String,
}

/// `asJson(structured)` (io.ts:275): the text is `json_line(structured, 2)`.
pub fn as_json(structured: Value) -> CallToolResult {
    let text = json_line(&structured, Some(2));
    tool_result(text, structured)
}

/// `asText(text, structured)` (io.ts:276).
pub fn as_text(text: &str, structured: Value) -> CallToolResult {
    tool_result(text.to_owned(), structured)
}

fn tool_result(text: String, structured: Value) -> CallToolResult {
    CallToolResult { content: vec![TextContent { kind: "text".to_owned(), text }], structured_content: Some(structured), is_error: None }
}

/// `turnText(out)` (io.ts:279): `notice`, then `AFTER_CUT_LINE` where `after_cut`, then the text or "", the absent
/// ones left out, joined by newlines.
pub fn turn_text(out: &TurnOut) -> String {
    let lines: Vec<Option<&str>> =
        vec![out.notice.as_deref(), (out.after_cut == Some(true)).then_some(AFTER_CUT_LINE), Some(out.text.as_deref().unwrap_or(""))];
    lines.into_iter().flatten().collect::<Vec<_>>().join("\n")
}

/// `noticed(out, said)` (io.ts:282): `notice` set to `said` where given, kept as it was where not.
pub fn noticed(out: TurnOut, said: Option<&str>) -> TurnOut {
    match said {
        None => out,
        Some(said) => TurnOut { notice: Some(said.to_owned()), ..out },
    }
}

/// `detachedOut(turn, opened, sub, said)` (io.ts:285-288).
pub fn detached_out(turn: &Turn, opened: Option<OpenedFn<'_>>, sub: Option<&SubOut>, said: Option<&str>) -> CallToolResult {
    let line: Vec<String> = [said.map(str::to_owned), Some(opened_thread_line(&turn.thread_id, opened, turn.session.cwd.as_deref()))]
        .into_iter()
        .flatten()
        .collect();
    let line = line.join("\n");
    let text = match &turn.session.capped {
        Some(capped) if turn.outcome == SessionStartOutcome::Held => format!("{line}\n{}", cap_wait_line(capped)),
        _ => line,
    };
    let mut structured = match serde_json::to_value(turn_view(turn)) {
        Ok(Value::Object(view)) => view,
        _ => Map::new(),
    };
    if let Some(sub) = sub {
        structured.insert("sub".to_owned(), serde_json::to_value(sub).unwrap_or(Value::Null));
    }
    if let Some(said) = said {
        structured.insert("notice".to_owned(), json!(said));
    }
    as_text(&text, Value::Object(structured))
}

/// `endView(ended)` (io.ts:290-300).
pub fn end_view(ended: &Ended) -> ThreadEndOut {
    ThreadEndOut {
        thread_id: ended.thread_id.clone(),
        status: ended.result.status,
        duration_ms: ended.result.duration_ms.map(|ms| ms.0),
        cost_usd: ended.result.cost_usd.map(|usd| usd.0),
        reply: notify_tail(&ended.result),
        result: ended.sub.clone(),
    }
}

pub use wsp_words::words::turn::NotifyLength;

/// `waitAnswer(named, waited, length)` (io.ts:305-308): the value and the line.
pub fn wait_answer(named: &[ThreadView], waited: &Waited, length: NotifyLength) -> (WaitOut, String) {
    match waited {
        Waited::Ended(ended) => {
            (WaitOut { finished: Some(end_view(ended)), timed_out: None }, notify_line(&ended.thread_id, &ended.result, Some(length)))
        }
        Waited::TimedOutMs(ms) => {
            let ids: Vec<String> = named.iter().map(thread_id_of).collect();
            (WaitOut { finished: None, timed_out: Some(true) }, wait_timed_out_line(&ids, *ms))
        }
    }
}

/// `timeoutFlag(value)` (io.ts:311-316): None for None; JS `Number(value)` finite and above zero, as milliseconds
/// (`seconds * 1000`); else refused.
pub fn timeout_flag(value: Option<&str>) -> Result<Option<f64>, HostError> {
    let Some(value) = value else { return Ok(None) };
    let seconds = to_number(value);
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(usage_refusal(
            &format!("--timeout takes seconds, a number above zero, and got {}.", quoted(value)),
            "Write it as --timeout <seconds>.",
        ));
    }
    Ok(Some(seconds * 1_000.0))
}

/// `turnOut(turn)` (io.ts:319-323): the turn's refusal where it did not complete, else its view.
pub fn turn_out(turn: &Turn) -> Result<TurnOut, HostError> {
    match turn_refusal(turn) {
        Some(failure) => Err(failure),
        None => Ok(turn_view(turn)),
    }
}

/// `PLAN_ONLY_TOOL` (io.ts:326).
pub const PLAN_ONLY_TOOL: &str =
    "nothing imported; call import again with yes true to take these defaults, or keep and cut per secret-shaped row";

// ---------------------------------------------------------------------------------------------------------------
// The tool inputs several verbs share (io.ts:328-405), each as the field it is, under its tool's name for it.
// ---------------------------------------------------------------------------------------------------------------

fn input(name: &'static str, kind: InputKind, optional: bool, describe: &str) -> ToolInput {
    ToolInput { name, kind, optional, describe: Some(describe.to_owned()) }
}

/// `WorkspaceIn` (io.ts:328).
pub fn workspace_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, false, "the workspace's name, or its id when two share a name")
}
/// `ThreadIn` (io.ts:329).
pub fn thread_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, false, "the thread, by its id or a prefix of it that names one, as threads lists them")
}
/// `AgentIn` (io.ts:330).
pub fn agent_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::String,
        true,
        &format!("the agent to run in the thread, one of {}; absent means the host's default", THREAD_AGENTS.join(", ")),
    )
}
/// `NotifyIn` (io.ts:331-337): a list of at least one string, optional.
pub fn notify_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::StringList { min: Some(1) },
        true,
        &format!(
            "who is told each time a turn of the new thread ends, each one a thread (by id, or a prefix of it) the line goes into as a message carrying that turn's whole reply, or me, which is the thread this call came out of when it came out of one and otherwise the person's app, where the line is its last reply line alone. Several targets each get the line once, which is how a builder's end reaches its orchestrator and a reviewer together; a target whose thread is gone by then falls back to the person, and the new thread's own id is refused. {NOTIFY_CALLER}."
        ),
    )
}
/// `RunProjectIn` (io.ts:338-346), its describe with the cloud span still in it: `in_cloud_words` cuts it where the
/// TS cuts it at load.
pub fn run_project_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::String,
        true,
        "the project the thread works on, by the name or the id projects lists<!-- cloud -->; one on a cloud gets a new machine forked from the image for the thread, and a machine named in its place runs it there as before<!-- /cloud -->. Absent from a thread, the thread runs beside the one asking, in its folder",
    )
}
/// `BesideIn` (io.ts:347).
pub fn beside_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a thread, by its id or a prefix of it, beside which the new thread runs: in the folder that thread works in, on the computer it runs on; never with project or branch")
}
/// `BranchIn` (io.ts:348).
pub fn branch_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a branch other than the one the project's folder has checked out: the thread runs in the worktree holding it, made under wsp's folder from the project folder's current commit for a new branch")
}
/// `OnIn` (io.ts:349-352).
pub fn on_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "boat, to start the thread as a sub-thread of the thread asking on a machine of its own on Boat, forked from the person's image. It fetches your pushed branch, or the ref from names, and pushes a branch of its own; without from your branch must be pushed first. It carries the thread's own project, which project may name or leave out; another project is refused. Never with beside, fork, at, cwd, resume, copy or replaces")
}
/// `SubSizeIn` (io.ts:353-356).
pub fn sub_size_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "with on, the machine's size as <cpu>x<memGb>: 2x4, 4x8, 8x16 or 16x32, those the person's plan sells; Boat's own size setting without it. A thread that builds or runs a test suite wants 4x8")
}
/// `SubFromIn` (io.ts:357-360).
pub fn sub_from_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "with on, a branch, tag or commit the remote already holds (origin/main, main, v1.2, a commit id) that the sub-thread starts from instead of your branch, which then need not be pushed; a ref the remote does not hold is refused naming it")
}
/// `RunCwdIn` (io.ts:361).
pub fn run_cwd_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a folder inside the project or one of its worktrees, absolute, which the thread starts in")
}
/// `SHARED_WITH` (io.ts:363): the `sharedWith` output field's name.
pub const SHARED_WITH: &str = "sharedWith";
/// `ExecCwdIn` (io.ts:364).
pub fn exec_cwd_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "the folder the command runs in, absolute; absent means the folder the thread works in")
}
/// `CwdIn` (io.ts:365).
pub fn cwd_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "the folder on the machine the thread works in or the command runs in, absolute. Absent means the workspace's own project, which is where a thread there starts unless it says otherwise")
}
/// `DetachIn` (io.ts:366).
pub fn detach_in(name: &'static str) -> ToolInput {
    input(name, InputKind::Boolean, true, &format!("true answers with the thread id the moment the turn is started, without the reply, and the turn's end reaches whoever notify named; for a turn that runs for minutes or an hour, so this call does not block for it. {NOTIFY_CALLER}."))
}
/// `ResumeIn` (io.ts:367-370).
pub fn resume_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a conversation the agent kept in the project's folder outside wsp, by the id conversations lists: the new thread opens on it, in the folder it ran in, under its own name, with its newest messages written above the message, and nothing of wsp's is written into the agent's own store until the thread is renamed here. Refused with beside, branch or cwd, since the conversation names its folder, where the agent holds no such conversation, where it is a wsp thread already, and where another app holds it open, unless copy")
}
/// `CopyIn` (io.ts:371).
pub fn copy_in(name: &'static str) -> ToolInput {
    input(name, InputKind::Boolean, true, "with resume, true runs the thread on a copy of the conversation the agent makes (Claude Code's --fork-session, Codex's thread/fork), the original left byte for byte as it was: the answer to one open in another app")
}
/// `TitleIn` (io.ts:372).
pub fn title_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "the thread's name, as a person's: it shows in the sidebar and in the agent's own list from the first second, and the title the host asks the agent for as the turn starts never replaces it; absent lets the thread be titled by its opening words until, seconds in, the agent names it")
}
/// `ForkIn` (io.ts:373).
pub fn fork_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a thread to fork, by id or a prefix of it, as threads lists them: the new thread runs on that thread's agent, at its model and effort unless named, with its conversation through one finished turn and nothing after, in that thread's folder, or with branch in a new worktree on that branch whose files are that turn's checkpoint; never with project, beside, cwd or replaces, and an agent other than the thread's is refused")
}
/// `AtIn` (io.ts:374): an integer of at least 1, optional.
pub fn at_in(name: &'static str) -> ToolInput {
    input(name, InputKind::Integer { min: Some(1) }, true, "with fork, the finished turn the fork carries the conversation through, counted from 1 in the order thread_read lists them; absent, the thread's latest finished turn")
}
/// `ReplacesIn` (io.ts:375).
pub fn replaces_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "a stopped or failed thread this new thread restarts, by id or a prefix of it, as threads lists them: the new thread's row names it, and once the new one starts the host settles it, so the person sees one row for the job; a restart the person starts of a child stands under that child's lead. Refused while that thread or one under it is working, for a thread that already has a restart (name that restart instead), and from a thread for one that is not under it, each before a machine is forked or woken")
}
/// `FilesIn` (io.ts:376-381), naming `FILES_MAX`, the image types and the two size caps.
pub fn files_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::StringList { min: None },
        true,
        &format!("paths on this computer, absolute or relative to the folder wsp runs in, of files to send with the message, at most {FILES_MAX}. An image ({IMAGE_TYPE_WORDS}, up to {IMAGE_MAX_WORDS}) goes to the agent as an image; any other file, up to {FILE_MAX_WORDS}, lands in the folder the thread works in, under .wsp-files where git lists none of it, and the message names its path. The host reads each file and sends its bytes, so the machine never reaches back for this computer\u{2019}s files; an image to an agent that reads none is refused naming that agent."),
    )
}
/// `FastIn` (io.ts:382).
pub fn fast_in(name: &'static str) -> ToolInput {
    input(name, InputKind::Boolean, true, "true runs the turn in the agent's fast mode, on a model that offers one, and is refused naming the model otherwise; absent runs at the agent's usual speed")
}
/// `ConfirmIn` (io.ts:383).
pub fn confirm_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::Boolean,
        true,
        "true deletes it; absent or false answers with what would go and deletes nothing, so a person can be asked first",
    )
}
/// `ACCESS_IN_WORDS` (io.ts:385-386).
pub const ACCESS_IN_WORDS: &str = "how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full";
/// `PICK_INPUTS` (io.ts:388-392): model, effort, access, in that order.
pub fn pick_inputs() -> Vec<ToolInput> {
    vec![
        input(
            "model",
            InputKind::String,
            true,
            "the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own",
        ),
        input(
            "effort",
            InputKind::String,
            true,
            "the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own",
        ),
        input("access", InputKind::String, true, ACCESS_IN_WORDS),
    ]
}
/// `SEND_INPUTS` (io.ts:394): model and effort as `PICK_INPUTS` words them.
pub fn send_inputs() -> Vec<ToolInput> {
    pick_inputs().into_iter().filter(|field| field.name != "access").collect()
}
/// `SizeIn` (io.ts:396).
pub fn size_in(name: &'static str) -> ToolInput {
    input(name, InputKind::String, true, "the machine size as <cpu>x<memGb>, like 2x4; absent takes the image's size. A size the provider does not offer is refused with the list it does, so read that list rather than guessing twice; a build wants the largest memory offered")
}
/// `MaxMachinesIn` (io.ts:397): an integer of at least 0, optional.
pub fn max_machines_in(name: &'static str) -> ToolInput {
    input(name, InputKind::Integer { min: Some(0) }, true, "how many machines may stand at once under one root thread; defaults to 3")
}
/// `MaxDepthIn` (io.ts:398): an integer of at least 1, optional.
pub fn max_depth_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::Integer { min: Some(1) },
        true,
        "how many levels deep the tree under a root thread may go, 2 by default; 1 stops at the root's own children",
    )
}
/// `PROJECT_FOLDERS` (io.ts:400).
pub fn project_folders_in(name: &'static str) -> ToolInput {
    input(
        name,
        InputKind::StringList { min: None },
        true,
        "folders on this computer, absolute, to weigh the histories by: only sessions that ran in one of them or under it count",
    )
}
/// `WEIGH_BY_FOLDERS` (io.ts:402).
pub fn weigh_by_folders_in(name: &'static str) -> ToolInput {
    input(name, InputKind::StringList { min: None }, true, "folders on this computer, absolute, to weigh the histories by: only sessions that ran in one of them or under it count. Naming one re-decides every tick from the rule, as tick does, so any flip an earlier call made goes")
}

/// `projectFolders(folders)` (io.ts:405): each through `absolute_path("project is a folder on this computer", _)`,
/// the first relative one refused.
pub fn project_folders(folders: &[String]) -> Result<Vec<String>, HostError> {
    folders.iter().map(|folder| absolute_path("project is a folder on this computer", folder)).collect()
}

/// `schemeFlag(value)` (io.ts:408-413).
pub fn scheme_flag(value: Option<&str>) -> Result<Option<TerminalScheme>, HostError> {
    let Some(value) = value else { return Ok(None) };
    match value {
        "light" => Ok(Some(TerminalScheme::Light)),
        "dark" => Ok(Some(TerminalScheme::Dark)),
        _ => Err(usage_refusal(
            &format!("--scheme takes one of {}, and got {}.", super::TERMINAL_SCHEMES.join(", "), quoted(value)),
            "Name one of those.",
        )),
    }
}

/// `projectsFlag(flags)` (io.ts:416): each `--project` resolved against `cwd` as node's `path.resolve` does; None
/// where the flag is not a list.
pub fn projects_flag(flags: &Flags, cwd: &str) -> Option<Vec<String>> {
    match flags.get("project") {
        Some(FlagValue::List(projects)) => Some(projects.iter().map(|project| resolve_path(cwd, project)).collect()),
        _ => None,
    }
}

/// `path.resolve(cwd, path)`: an absolute path stands alone, a relative one is joined under `cwd`, and `.` and `..`
/// are folded away.
pub fn resolve_path(cwd: &str, path: &str) -> String {
    let joined = if path.starts_with('/') {
        path.to_owned()
    } else if path.is_empty() {
        cwd.to_owned()
    } else {
        format!("{cwd}/{path}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            kept => parts.push(kept),
        }
    }
    format!("/{}", parts.join("/"))
}

/// `progress(io)` (io.ts:419): `log` to stdout, `note` to stderr.
pub struct Progress<'a> {
    pub io: &'a dyn CliIo,
}

impl Progress<'_> {
    pub fn log(&self, line: &str) {
        self.io.log(line);
    }
    pub fn note(&self, line: &str) {
        self.io.error(line);
    }
}

/// `printTable(ctx, value, lines, next)` (io.ts:423-428): emit the value; under --json nothing more; else each line
/// of `lines(colour depth of stdout)` then `next`, through `io.log`.
pub fn print_table(ctx: &VerbContext, value: &Value, lines: &dyn Fn(u8) -> Vec<String>, next: &str) {
    ctx.out.emit(value, None);
    if flag_set(&ctx.flags, "json") {
        return;
    }
    for line in lines(colour_depth(ctx.io.is_tty(), &ctx.deps.env)) {
        ctx.io.log(&line);
    }
    ctx.io.log(next);
}

/// What `host_folders` is asked (io.ts:434).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FoldersAsked {
    pub folder: Option<String>,
    pub hidden: Option<bool>,
    pub repos: Option<bool>,
    pub on: Option<String>,
}

/// `hostFolders(client, asked, here)` (io.ts:434-446): `on` looked up first (one `places.list`); a folder on this
/// computer through `here`, on another computer absolute or refused naming it; then one `host.folders` with `dir`,
/// `hidden` only when true, `repos` only when true, `on` (the place's id) only for another computer.
pub async fn host_folders(
    client: &dyn HostClient,
    asked: &FoldersAsked,
    here: &(dyn Fn(&str) -> Result<String, HostError> + Send + Sync),
) -> Result<HostRow<HostFolderListing>, HostError> {
    let place = match asked.on.as_deref() {
        Some(on) => Some(place_named(client, on).await?),
        None => None,
    };
    let elsewhere = place.filter(|place| place.id != HERE_PLACE_ID);
    let dir = match asked.folder.as_deref() {
        None => None,
        Some(folder) => Some(match &elsewhere {
            None => here(folder)?,
            Some(place) => absolute_path(&format!("folder is a path on {}", place.name), folder)?,
        }),
    };
    let mut params = Frame::new();
    if let Some(dir) = dir {
        params.insert("dir".to_owned(), json!(dir));
    }
    if asked.hidden == Some(true) {
        params.insert("hidden".to_owned(), json!(true));
    }
    if asked.repos == Some(true) {
        params.insert("repos".to_owned(), json!(true));
    }
    if let Some(place) = &elsewhere {
        params.insert("on".to_owned(), json!(place.id));
    }
    let reply = client.request("host.folders", params).await?;
    HostRow::from_reply(&reply, "host.folders", "listing")
}

/// `initSetup(client)` (io.ts:449-452): `init.get {}`'s `setup`, parsed against `InitSetup`; a shape it refuses is
/// an error.
pub async fn init_setup(client: &dyn HostClient) -> Result<InitSetup, HostError> {
    let reply = client.request("init.get", Frame::new()).await?;
    let setup = reply.get("setup").cloned().unwrap_or(Value::Null);
    parse::<InitSetup>(&setup).map_err(|_| HostError::new(other_version("init.get")))
}

/// `folderLines(listing)` (io.ts:456-461).
pub fn folder_lines(listing: &HostFolderListing) -> Vec<String> {
    let mut rows = vec![vec!["FOLDER".to_owned(), "GIT".to_owned()]];
    rows.extend(listing.folders.iter().map(|folder| {
        vec![folder.path.clone(), if folder.repo { folder.branch.clone().unwrap_or_else(|| "git".to_owned()) } else { String::new() }]
    }));
    let mut lines = table(&rows);
    lines.push(format!(
        "{} Browsable: {}.",
        folder_level_line(&listing.dir, listing.folders.len(), listing.hidden.0 .0),
        listing.roots.join(", ")
    ));
    lines
}

/// `REASON_FLAG` (io.ts:463).
pub fn reason_flag() -> FlagTable {
    FlagTable(vec![("reason", FlagRow::STRING)])
}

/// The one reply field a socket answer carries, read into the type a caller takes; a field the reply lacks or shapes
/// otherwise is the other-version refusal the host's answer earns (client.ts `replyField`).
fn field_of<T: serde::de::DeserializeOwned>(reply: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    reply.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

fn pick_frame<const N: usize>(fields: [(&str, Value); N]) -> Frame {
    fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect()
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("a string writes as JSON")
}
