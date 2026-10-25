// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/io.ts` at 65f6b89c7: the roads that follow a pushed stream (exec, export, import), the
//! plan's lines, the one confirmation, the tool door's answers and output shapes, and the tool inputs several verbs
//! share. Spec: `spec/io.md`.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use wsp_wire::types::project_import::ProjectImportRequest;
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::project_bundle::{HostFolderListing, ProjectExportResult, ProjectImportResult, ProjectPlan, TerminalScheme};
use wsp_wire::views::session::{ThreadCapWait, ThreadView};
use wsp_wire::views::session_events::TurnStatus;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView};
use wsp_wire::wire::capabilities::InitSetup;
use wsp_wire::wire::runtime::{ExecEvent, SessionStartOutcome};
use wsp_words::exit::HostError;

use super::client::{CallToolResult, CliIo, FlagTable, Flags, HostClient, OpenedFn, ToolInput, VerbContext};
use super::turns_help::{Ended, Turn, Waited};

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
    let _ = (client, workspace_id, argv, cwd, on);
    todo!("io.ts:68-86")
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
    let _ = (client, workspace_id, req, on);
    todo!("io.ts:99-121")
}

/// `planProject(client, source)` (io.ts:124-126): `project.plan {source}`'s `plan`.
pub async fn plan_project(client: &dyn HostClient, source: &str) -> Result<ProjectPlan, HostError> {
    let _ = (client, source);
    todo!("io.ts:124-126")
}

/// `importProject(client, workspaceId, req, on)` (io.ts:130-143): as export, matched on `workspaceId`, `source` and
/// `dest`; the request is `{workspaceId, ...req}`.
pub async fn import_project(
    client: &dyn HostClient,
    workspace_id: &str,
    req: &ProjectImportRequest,
    on: StageSink,
) -> Result<ProjectImportResult, HostError> {
    let _ = (client, workspace_id, req, on);
    todo!("io.ts:130-143")
}

// ---------------------------------------------------------------------------------------------------------------
// The plan (io.ts:147-197).
// ---------------------------------------------------------------------------------------------------------------

/// `secretsChosen(plan, keep, cut)` (io.ts:147-156): every keep and cut path must be a secret the plan lists, keeps
/// first then cuts, the first stranger refused; then the plan's default ticks, plus keep, less cut.
pub fn secrets_chosen(plan: &ProjectPlan, keep: &[String], cut: &[String]) -> Result<BTreeSet<String>, HostError> {
    let _ = (plan, keep, cut);
    todo!("io.ts:147-156")
}

/// `agentsChosen(plan, named)` (io.ts:159-166): None is the plan's default; each named agent must travel.
pub fn agents_chosen(plan: &ProjectPlan, named: Option<&[String]>) -> Result<BTreeSet<String>, HostError> {
    let _ = (plan, named);
    todo!("io.ts:159-166")
}

/// `planLines(plan, ticked, agents, workspace)` (io.ts:171-185).
pub fn plan_lines(
    plan: &ProjectPlan,
    ticked: &BTreeSet<String>,
    agents: &BTreeSet<String>,
    workspace: Option<&WorkspaceView>,
) -> Vec<String> {
    let _ = (plan, ticked, agents, workspace);
    todo!("io.ts:171-185")
}

/// `PLAN_ONLY` (io.ts:188).
pub const PLAN_ONLY: &str =
    "nothing imported; run again with --yes to take these defaults, or --keep <path> and --cut <path> per secret-shaped row";

/// `IMPORT_NOW` (io.ts:191).
pub const IMPORT_NOW: &str = "Import now? y/N";

/// `withReplaceHint(e)` (io.ts:194-197): an error of kind `exists` gains `\nRun again with --replace to overwrite
/// it.` and keeps the kind; anything else as it came.
pub fn with_replace_hint(e: HostError) -> HostError {
    let _ = e;
    todo!("io.ts:194-197")
}

/// `agentsFlag(value)` (io.ts:200-208): None for None; split on `,`, each trimmed, empties dropped; none left is
/// refused; the first id the catalog does not know is refused naming the catalog's ids.
pub fn agents_flag(value: Option<&str>) -> Result<Option<Vec<String>>, HostError> {
    let _ = value;
    todo!("io.ts:200-208")
}

// ---------------------------------------------------------------------------------------------------------------
// The one confirmation (io.ts:212-223) and the quiet tool door (io.ts:226-229).
// ---------------------------------------------------------------------------------------------------------------

/// `confirmed(ctx, question, name)` (io.ts:212-214): `confirmed_at` with `--yes` off the line.
pub async fn confirmed(ctx: &VerbContext, question: &str, name: &str) -> Result<bool, HostError> {
    let _ = (ctx, question, name);
    todo!("io.ts:212-214")
}

/// `confirmedAt(io, yes, question, name)` (io.ts:217-223): yes is true; off a terminal a usage refusal
/// `<question's first line> There is no terminal to answer on.` with `Pass --yes to say yes.`, nothing asked; at a
/// terminal `ask`, "yes" is true, anything else prints `<name> kept` on stderr and is false.
pub async fn confirmed_at(io: &dyn CliIo, yes: bool, question: &str, name: &str) -> Result<bool, HostError> {
    let _ = (io, yes, question, name);
    todo!("io.ts:217-223")
}

/// `QUIET_LINE` (io.ts:228): the waking line dropped on the tool door.
pub fn quiet_line(line: &str) {
    let _ = line;
}

// ---------------------------------------------------------------------------------------------------------------
// The tool door's words and output shapes (io.ts:231-326).
// ---------------------------------------------------------------------------------------------------------------

/// `Created` (io.ts:231).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Created {
    pub workspace: WorkspaceOut,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub notice: Option<String>,
}

/// `SEND_MEETS` (io.ts:236), built from `SessionStartOutcome`'s words and `still_working_line()`.
pub fn send_meets() -> String {
    todo!("io.ts:235-236")
}

/// `CAP_MEETS` (io.ts:239).
pub fn cap_meets() -> String {
    todo!("io.ts:239")
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<SubResult>,
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
    let _ = structured;
    todo!("io.ts:275")
}

/// `asText(text, structured)` (io.ts:276).
pub fn as_text(text: &str, structured: Value) -> CallToolResult {
    let _ = (text, structured);
    todo!("io.ts:276")
}

/// `turnText(out)` (io.ts:279): `notice`, then `AFTER_CUT_LINE` where `after_cut`, then the text or "", the absent
/// ones left out, joined by newlines.
pub fn turn_text(out: &TurnOut) -> String {
    let _ = out;
    todo!("io.ts:279")
}

/// `noticed(out, said)` (io.ts:282): `notice` set to `said` where given, kept as it was where not.
pub fn noticed(out: TurnOut, said: Option<&str>) -> TurnOut {
    let _ = (out, said);
    todo!("io.ts:282")
}

/// `detachedOut(turn, opened, sub, said)` (io.ts:285-288).
pub fn detached_out(turn: &Turn, opened: Option<OpenedFn<'_>>, sub: Option<&SubOut>, said: Option<&str>) -> CallToolResult {
    let _ = (turn, opened, sub, said);
    todo!("io.ts:285-288")
}

/// `endView(ended)` (io.ts:290-300).
pub fn end_view(ended: &Ended) -> ThreadEndOut {
    let _ = ended;
    todo!("io.ts:290-300")
}

pub use wsp_words::words::turn::NotifyLength;

/// `waitAnswer(named, waited, length)` (io.ts:305-308): the value and the line.
pub fn wait_answer(named: &[ThreadView], waited: &Waited, length: NotifyLength) -> (WaitOut, String) {
    let _ = (named, waited, length);
    todo!("io.ts:305-308")
}

/// `timeoutFlag(value)` (io.ts:311-316): None for None; JS `Number(value)` finite and above zero, as milliseconds
/// (`seconds * 1000`); else refused.
pub fn timeout_flag(value: Option<&str>) -> Result<Option<f64>, HostError> {
    let _ = value;
    todo!("io.ts:311-316")
}

/// `turnOut(turn)` (io.ts:319-323): the turn's refusal where it did not complete, else its view.
pub fn turn_out(turn: &Turn) -> Result<TurnOut, HostError> {
    let _ = turn;
    todo!("io.ts:319-323")
}

/// `PLAN_ONLY_TOOL` (io.ts:326).
pub const PLAN_ONLY_TOOL: &str =
    "nothing imported; call import again with yes true to take these defaults, or keep and cut per secret-shaped row";

// ---------------------------------------------------------------------------------------------------------------
// The tool inputs several verbs share (io.ts:328-405), each as the field it is, under its tool's name for it.
// ---------------------------------------------------------------------------------------------------------------

/// `WorkspaceIn` (io.ts:328).
pub fn workspace_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:328")
}
/// `ThreadIn` (io.ts:329).
pub fn thread_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:329")
}
/// `AgentIn` (io.ts:330).
pub fn agent_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:330")
}
/// `NotifyIn` (io.ts:331-337): a list of at least one string, optional.
pub fn notify_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:331-337")
}
/// `RunProjectIn` (io.ts:338-346), its describe with the cloud span still in it: `in_cloud_words` cuts it where the
/// TS cuts it at load.
pub fn run_project_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:338-346")
}
/// `BesideIn` (io.ts:347).
pub fn beside_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:347")
}
/// `BranchIn` (io.ts:348).
pub fn branch_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:348")
}
/// `OnIn` (io.ts:349-352).
pub fn on_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:349-352")
}
/// `SubSizeIn` (io.ts:353-356).
pub fn sub_size_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:353-356")
}
/// `SubFromIn` (io.ts:357-360).
pub fn sub_from_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:357-360")
}
/// `RunCwdIn` (io.ts:361).
pub fn run_cwd_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:361")
}
/// `SHARED_WITH` (io.ts:363): the `sharedWith` output field's name.
pub const SHARED_WITH: &str = "sharedWith";
/// `ExecCwdIn` (io.ts:364).
pub fn exec_cwd_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:364")
}
/// `CwdIn` (io.ts:365).
pub fn cwd_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:365")
}
/// `DetachIn` (io.ts:366).
pub fn detach_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:366")
}
/// `ResumeIn` (io.ts:367-370).
pub fn resume_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:367-370")
}
/// `CopyIn` (io.ts:371).
pub fn copy_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:371")
}
/// `TitleIn` (io.ts:372).
pub fn title_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:372")
}
/// `ForkIn` (io.ts:373).
pub fn fork_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:373")
}
/// `AtIn` (io.ts:374): an integer of at least 1, optional.
pub fn at_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:374")
}
/// `ReplacesIn` (io.ts:375).
pub fn replaces_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:375")
}
/// `FilesIn` (io.ts:376-381), naming `FILES_MAX`, the image types and the two size caps.
pub fn files_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:376-381")
}
/// `FastIn` (io.ts:382).
pub fn fast_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:382")
}
/// `ConfirmIn` (io.ts:383).
pub fn confirm_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:383")
}
/// `ACCESS_IN_WORDS` (io.ts:385-386).
pub const ACCESS_IN_WORDS: &str = "how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full";
/// `PICK_INPUTS` (io.ts:388-392): model, effort, access, in that order.
pub fn pick_inputs() -> Vec<ToolInput> {
    todo!("io.ts:388-392")
}
/// `SEND_INPUTS` (io.ts:394): model and effort as `PICK_INPUTS` words them.
pub fn send_inputs() -> Vec<ToolInput> {
    todo!("io.ts:394")
}
/// `SizeIn` (io.ts:396).
pub fn size_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:396")
}
/// `MaxMachinesIn` (io.ts:397): an integer of at least 0, optional.
pub fn max_machines_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:397")
}
/// `MaxDepthIn` (io.ts:398): an integer of at least 1, optional.
pub fn max_depth_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:398")
}
/// `PROJECT_FOLDERS` (io.ts:400).
pub fn project_folders_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:400")
}
/// `WEIGH_BY_FOLDERS` (io.ts:402).
pub fn weigh_by_folders_in(name: &'static str) -> ToolInput {
    let _ = name;
    todo!("io.ts:402")
}

/// `projectFolders(folders)` (io.ts:405): each through `absolute_path("project is a folder on this computer", _)`,
/// the first relative one refused.
pub fn project_folders(folders: &[String]) -> Result<Vec<String>, HostError> {
    let _ = folders;
    todo!("io.ts:405")
}

/// `schemeFlag(value)` (io.ts:408-413).
pub fn scheme_flag(value: Option<&str>) -> Result<Option<TerminalScheme>, HostError> {
    let _ = value;
    todo!("io.ts:408-413")
}

/// `projectsFlag(flags)` (io.ts:416): each `--project` resolved against `cwd` as node's `path.resolve` does; None
/// where the flag is not a list.
pub fn projects_flag(flags: &Flags, cwd: &str) -> Option<Vec<String>> {
    let _ = (flags, cwd);
    todo!("io.ts:416")
}

/// `progress(io)` (io.ts:419): `log` to stdout, `note` to stderr.
pub struct Progress<'a> {
    pub io: &'a dyn CliIo,
}

impl Progress<'_> {
    pub fn log(&self, line: &str) {
        let _ = line;
        todo!("io.ts:419")
    }
    pub fn note(&self, line: &str) {
        let _ = line;
        todo!("io.ts:419")
    }
}

/// `printTable(ctx, value, lines, next)` (io.ts:423-428): emit the value; under --json nothing more; else each line
/// of `lines(colour depth of stdout)` then `next`, through `io.log`.
pub fn print_table(ctx: &VerbContext, value: &Value, lines: &dyn Fn(u8) -> Vec<String>, next: &str) {
    let _ = (ctx, value, lines, next);
    todo!("io.ts:423-428")
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
) -> Result<HostFolderListing, HostError> {
    let _ = (client, asked, here);
    todo!("io.ts:434-446")
}

/// `initSetup(client)` (io.ts:449-452): `init.get {}`'s `setup`, parsed against `InitSetup`; a shape it refuses is
/// an error.
pub async fn init_setup(client: &dyn HostClient) -> Result<InitSetup, HostError> {
    let _ = client;
    todo!("io.ts:449-452")
}

/// `folderLines(listing)` (io.ts:456-461).
pub fn folder_lines(listing: &HostFolderListing) -> Vec<String> {
    let _ = listing;
    todo!("io.ts:456-461")
}

/// `REASON_FLAG` (io.ts:463).
pub fn reason_flag() -> FlagTable {
    todo!("io.ts:463")
}
