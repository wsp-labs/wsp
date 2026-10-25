// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts lines 1-1029: the RuntimeOp union every client frame is read by,
//! the envelope beside it, and its reader. The caps of each variant are listed at the foot of runtime.rs.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::agents_report::{AgentsTarget, McpScope};
use crate::types::attachments::Attachment;
use crate::types::conversations::ResumeAsk;
use crate::types::init_job::{InitRoad, InitScreenId};
use crate::types::project_look::ProjectIconHash;
use crate::types::pull_request::{MergeMethod, PullRequestItem, ReactionContent};
use crate::types::recipe_file::RecipeFile;
use crate::types::slate_wire::SlateWireValues;
use crate::types::thread_defaults::AccessChoice;
use crate::types::usage::{ReadingsRange, UsageRange, UsageSplit};
use crate::types::workspace_look::{WorkspaceGlyph, WorkspaceTheme};
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::activity::{ActivityEnded, ProcessesAsk};
use crate::views::app_procs::AppProcList;
use crate::views::golden_image::MachineKind;
use crate::views::place::{PlaceSettingWord, PlaceSetupStep, SudoPassword};
use crate::views::preferences::{EditorId, PreferencesPatch};
use crate::views::project_bundle::TerminalScheme;
use crate::views::session::{ForkSource, SessionOrigin, SessionView, ThreadMarks};
use crate::views::session_events::RunState;
use crate::views::workspace::{SeedChoice, WorkspaceOrigin};
use crate::wire::devices::{DaemonFrame, TicketPurpose};
use crate::wire::limits::RelayPort;
use crate::wire::place_link::{DeviceAuthRequest, PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, SealOpenRequest};
use wsp_frames::{False, True};

use super::runtime::*;
use crate::views::workspace::EventAsker;

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum RuntimeOp {
    #[serde(rename = "auth", rename_all = "camelCase")]
    Auth { id: ReqId, token: String },
    #[serde(rename = "ticket.issue", rename_all = "camelCase")]
    TicketIssue { id: ReqId, purpose: TicketPurpose },
    #[serde(rename = "pair.issue", rename_all = "camelCase")]
    PairIssue { id: ReqId, here: Option<True> },
    #[serde(rename = "pair.redeem", rename_all = "camelCase")]
    PairRedeem { id: ReqId, code: String, name: String },
    #[serde(rename = "seal.open")]
    SealOpen(SealOpenRequest),
    #[serde(rename = "device.auth")]
    DeviceAuth(DeviceAuthRequest),
    #[serde(rename = "account.get", rename_all = "camelCase")]
    AccountGet { id: ReqId },
    #[serde(rename = "devices.list", rename_all = "camelCase")]
    DevicesList { id: ReqId },
    #[serde(rename = "devices.revoke", rename_all = "camelCase")]
    DevicesRevoke { id: ReqId, device_id: String },
    #[serde(rename = "place.join")]
    PlaceJoin(PlaceJoinRequest),
    #[serde(rename = "place.auth")]
    PlaceAuth(PlaceAuthRequest),
    #[serde(rename = "place.prove")]
    PlaceProve(PlaceProveRequest),
    #[serde(rename = "places.list", rename_all = "camelCase")]
    PlacesList { id: ReqId },
    #[serde(rename = "places.update", rename_all = "camelCase")]
    PlacesUpdate { id: ReqId, place_id: String, sudo_password: Option<SudoPassword> },
    #[serde(rename = "places.holds", rename_all = "camelCase")]
    PlacesHolds { id: ReqId, place_id: String },
    #[serde(rename = "places.remove", rename_all = "camelCase")]
    PlacesRemove { id: ReqId, place_id: String, sudo_password: Option<SudoPassword>, force: Option<bool>, forget: Option<bool> },
    #[serde(rename = "places.doctor", rename_all = "camelCase")]
    PlacesDoctor { id: ReqId, place_id: String, doctor_id: String, project: Option<String> },
    #[serde(rename = "places.dial", rename_all = "camelCase")]
    PlacesDial { id: ReqId, place_id: String },
    #[serde(rename = "places.set", rename_all = "camelCase")]
    PlacesSet {
        id: ReqId,
        place_id: String,
        reset: Option<Vec<PlaceSettingWord>>,
        name: Option<String>,
        ssh: Option<String>,
        recipe: Option<String>,
        threads: Option<i64>,
        machines: Option<i64>,
        spend_per_day_usd: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        nap_ms: Option<Option<i64>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        turn_limit_ms: Option<Option<i64>>,
        spawn: Option<PlacesSetSpawn>,
        size: Option<PlacesSetSize>,
        delete_after_ms: Option<i64>,
    },
    #[serde(rename = "places.follow", rename_all = "camelCase")]
    PlacesFollow { id: ReqId, place_id: String, recipe: String },
    #[serde(rename = "places.skip", rename_all = "camelCase")]
    PlacesSkip { id: ReqId, place_id: String, row: String },
    #[serde(rename = "places.setupLog", rename_all = "camelCase")]
    PlacesSetupLog { id: ReqId, place_id: String, step: Option<PlaceSetupStep> },
    #[serde(rename = "places.estimate", rename_all = "camelCase")]
    PlacesEstimate { id: ReqId, r#ref: String, choices: RecipeFile },
    #[serde(rename = "places.loginLanded", rename_all = "camelCase")]
    PlacesLoginLanded { id: ReqId, place_id: String, agent: String },
    #[serde(rename = "places.door", rename_all = "camelCase")]
    PlacesDoor { id: ReqId },
    #[serde(rename = "places.mint", rename_all = "camelCase")]
    PlacesMint { id: ReqId },
    #[serde(rename = "places.sshHosts", rename_all = "camelCase")]
    PlacesSshHosts { id: ReqId },
    #[serde(rename = "places.add", rename_all = "camelCase")]
    PlacesAdd {
        id: ReqId,
        add_id: Option<String>,
        address: String,
        name: Option<String>,
        ssh_port: Option<i64>,
        key_path: Option<String>,
        host_key: Option<String>,
        sudo_password: Option<SudoPassword>,
        recipe: Option<String>,
    },
    #[serde(rename = "places.setup", rename_all = "camelCase")]
    PlacesSetup {
        id: ReqId,
        r#ref: String,
        recipe: Option<String>,
        choices: Option<RecipeFile>,
        add_id: Option<String>,
        row: Option<String>,
    },
    #[serde(rename = "places.choose", rename_all = "camelCase")]
    PlacesChoose { id: ReqId, r#ref: String, choices: RecipeFile, recipe: Option<String> },
    #[serde(rename = "recipes.list", rename_all = "camelCase")]
    RecipesList { id: ReqId },
    #[serde(rename = "recipes.get", rename_all = "camelCase")]
    RecipesGet { id: ReqId, name: String },
    #[serde(rename = "recipes.save", rename_all = "camelCase")]
    RecipesSave { id: ReqId, name: String, file: Option<Value>, from: Option<String> },
    #[serde(rename = "recipes.remove", rename_all = "camelCase")]
    RecipesRemove { id: ReqId, name: String, check: Option<bool> },
    #[serde(rename = "recipes.options", rename_all = "camelCase")]
    RecipesOptions { id: ReqId },
    #[serde(rename = "events.subscribe", rename_all = "camelCase")]
    EventsSubscribe { id: ReqId, after: Option<i64>, stream: Option<String>, creates: Option<bool> },
    #[serde(rename = "status.subscribe", rename_all = "camelCase")]
    StatusSubscribe { id: ReqId },
    #[serde(rename = "status.list", rename_all = "camelCase")]
    StatusList { id: ReqId },
    #[serde(rename = "workspaces.create", rename_all = "camelCase")]
    WorkspacesCreate {
        id: ReqId,
        golden: Option<String>,
        project: String,
        name: String,
        cpu: Option<JsNumber>,
        mem_mb: Option<JsNumber>,
        envs: Option<IndexMap<String, String>>,
        labels: Option<IndexMap<String, String>>,
        agents: Option<WorkspacesCreateAgents>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        idle_window_ms: Option<Option<JsNumber>>,
        parent: Option<String>,
        engine: Option<bool>,
        sub: Option<WorkspacesCreateSub>,
    },
    #[serde(rename = "workspaces.landing", rename_all = "camelCase")]
    WorkspacesLanding { id: ReqId, project: String, sized: Option<True> },
    #[serde(rename = "workspaces.list", rename_all = "camelCase")]
    WorkspacesList { id: ReqId },
    #[serde(rename = "workspaces.resolve", rename_all = "camelCase")]
    WorkspacesResolve { id: ReqId, r#ref: String },
    #[serde(rename = "workspaces.get", rename_all = "camelCase")]
    WorkspacesGet { id: ReqId, workspace_id: String, thread_id: Option<String> },
    #[serde(rename = "workspaces.nap", rename_all = "camelCase")]
    WorkspacesNap { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.resize", rename_all = "camelCase")]
    WorkspacesResize { id: ReqId, workspace_id: String, size: WorkspacesResizeSize },
    #[serde(rename = "workspaces.wake", rename_all = "camelCase")]
    WorkspacesWake { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.restartDaemon", rename_all = "camelCase")]
    WorkspacesRestartDaemon { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.stopWake", rename_all = "camelCase")]
    WorkspacesStopWake { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.upgrade", rename_all = "camelCase")]
    WorkspacesUpgrade { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.rename", rename_all = "camelCase")]
    WorkspacesRename { id: ReqId, workspace_id: String, name: String },
    #[serde(rename = "workspaces.look", rename_all = "camelCase")]
    WorkspacesLook {
        id: ReqId,
        workspace_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        theme: Option<Option<WorkspaceTheme>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        glyph: Option<Option<WorkspaceGlyph>>,
    },
    #[serde(rename = "workspaces.bringBack", rename_all = "camelCase")]
    WorkspacesBringBack { id: ReqId, workspace_id: String, title: Option<String>, body: Option<String> },
    #[serde(rename = "worktree.make", rename_all = "camelCase")]
    WorktreeMake { id: ReqId, project: String, branch: String },
    #[serde(rename = "folder.make", rename_all = "camelCase")]
    FolderMake { id: ReqId, project: String },
    #[serde(rename = "worktree.remove", rename_all = "camelCase")]
    WorktreeRemove { id: ReqId, project: String, branch: String, force: Option<bool>, check: Option<bool> },
    #[serde(rename = "workspaces.checkout", rename_all = "camelCase")]
    WorkspacesCheckout { id: ReqId, workspace_id: String, fresh: Option<bool> },
    #[serde(rename = "workspaces.discard", rename_all = "camelCase")]
    WorkspacesDiscard { id: ReqId, workspace_id: String, path: String, check: Option<bool>, thread_id: Option<String> },
    #[serde(rename = "workspaces.commit", rename_all = "camelCase")]
    WorkspacesCommit { id: ReqId, workspace_id: String, message: String, paths: Option<Vec<String>>, thread_id: Option<String> },
    #[serde(rename = "workspaces.commitDraft", rename_all = "camelCase")]
    WorkspacesCommitDraft { id: ReqId, workspace_id: String, paths: Option<Vec<String>>, thread_id: Option<String> },
    #[serde(rename = "workspaces.viewed", rename_all = "camelCase")]
    WorkspacesViewed {
        id: ReqId,
        workspace_id: String,
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        blob: Option<Option<String>>,
    },
    #[serde(rename = "workspaces.pullRequestView", rename_all = "camelCase")]
    WorkspacesPullRequestView { id: ReqId, workspace_id: String, fresh: Option<bool> },
    #[serde(rename = "workspaces.pullRequestDiff", rename_all = "camelCase")]
    WorkspacesPullRequestDiff { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.pullRequestSend", rename_all = "camelCase")]
    WorkspacesPullRequestSend { id: ReqId, workspace_id: String, items: Vec<PullRequestItem>, thread_id: Option<String> },
    #[serde(rename = "workspaces.pullRequestReply", rename_all = "camelCase")]
    WorkspacesPullRequestReply { id: ReqId, workspace_id: String, reply_to: Option<i64>, thread_id: Option<String>, body: String },
    #[serde(rename = "workspaces.pullRequestResolve", rename_all = "camelCase")]
    WorkspacesPullRequestResolve { id: ReqId, workspace_id: String, thread_id: String, resolved: bool },
    #[serde(rename = "workspaces.pullRequestReact", rename_all = "camelCase")]
    WorkspacesPullRequestReact { id: ReqId, workspace_id: String, subject: String, content: ReactionContent, on: bool },
    #[serde(rename = "workspaces.fix", rename_all = "camelCase")]
    WorkspacesFix {
        id: ReqId,
        workspace_id: String,
        check: Option<String>,
        child: Option<String>,
        thread_id: Option<String>,
        child_thread_id: Option<String>,
    },
    #[serde(rename = "workspaces.merge", rename_all = "camelCase")]
    WorkspacesMerge {
        id: ReqId,
        workspace_id: String,
        method: Option<MergeMethod>,
        when_checks_pass: Option<bool>,
        head: Option<String>,
        thread_id: Option<String>,
    },
    #[serde(rename = "workspaces.start", rename_all = "camelCase")]
    WorkspacesStart {
        id: ReqId,
        url: String,
        project: Option<String>,
        agent: Option<String>,
        model: Option<String>,
        effort: Option<String>,
        access: Option<AccessChoice>,
    },
    #[serde(rename = "workspaces.review", rename_all = "camelCase")]
    WorkspacesReview {
        id: ReqId,
        url: Option<String>,
        workspace_id: Option<String>,
        agent: Option<String>,
        model: Option<String>,
        effort: Option<String>,
    },
    #[serde(rename = "workspaces.reviewDraft", rename_all = "camelCase")]
    WorkspacesReviewDraft {
        id: ReqId,
        workspace_id: String,
        summary: Option<String>,
        verdict: Option<WorkspacesReviewDraftVerdict>,
        on: Option<Vec<WorkspacesReviewDraftOnItem>>,
    },
    #[serde(rename = "workspaces.reviewPost", rename_all = "camelCase")]
    WorkspacesReviewPost { id: ReqId, workspace_id: String, thread_id: Option<String> },
    #[serde(rename = "workspaces.update", rename_all = "camelCase")]
    WorkspacesUpdate { id: ReqId, workspace_id: String, thread_id: Option<String> },
    #[serde(rename = "workspaces.mergeIn", rename_all = "camelCase")]
    WorkspacesMergeIn { id: ReqId, workspace_id: String, child: String, thread_id: Option<String>, child_thread_id: Option<String> },
    #[serde(rename = "workspaces.delete", rename_all = "camelCase")]
    WorkspacesDelete { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.forget", rename_all = "camelCase")]
    WorkspacesForget { id: ReqId, workspace_id: String },
    #[serde(rename = "workspaces.snapshot", rename_all = "camelCase")]
    WorkspacesSnapshot { id: ReqId, workspace_id: String },
    #[serde(rename = "projectGoldens.list", rename_all = "camelCase")]
    ProjectGoldensList { id: ReqId },
    #[serde(rename = "projectGoldens.remove", rename_all = "camelCase")]
    ProjectGoldensRemove { id: ReqId, snapshot_id: String },
    #[serde(rename = "workspaces.touch", rename_all = "camelCase")]
    WorkspacesTouch { id: ReqId, workspace_id: String },
    #[serde(rename = "daemon.open", rename_all = "camelCase")]
    DaemonOpen { id: ReqId, workspace_id: Option<String>, place_id: Option<String> },
    #[serde(rename = "sys.subscribe", rename_all = "camelCase")]
    SysSubscribe { id: ReqId, workspace_id: Option<String>, place_id: Option<String> },
    #[serde(rename = "sys.unsubscribe", rename_all = "camelCase")]
    SysUnsubscribe { id: ReqId, workspace_id: Option<String>, place_id: Option<String> },
    #[serde(rename = "appProcs.watch", rename_all = "camelCase")]
    AppProcsWatch { id: ReqId },
    #[serde(rename = "appProcs.unwatch", rename_all = "camelCase")]
    AppProcsUnwatch { id: ReqId },
    #[serde(rename = "appProcs.offer", rename_all = "camelCase")]
    AppProcsOffer { id: ReqId },
    #[serde(rename = "appProcs.set", rename_all = "camelCase")]
    AppProcsSet { id: ReqId, procs: AppProcList },
    #[serde(rename = "activity.read", rename_all = "camelCase")]
    ActivityRead { id: ReqId, computer: Option<String>, thread: Option<String> },
    #[serde(rename = "activity.end", rename_all = "camelCase")]
    ActivityEnd { id: ReqId, computer: Option<String>, pid: i64, started_at: JsNumber },
    #[serde(rename = "daemon.send", rename_all = "camelCase")]
    DaemonSend { id: ReqId, channel: String, frame: DaemonFrame },
    #[serde(rename = "daemon.close", rename_all = "camelCase")]
    DaemonClose { id: ReqId, channel: String },
    #[serde(rename = "sessions.start", rename_all = "camelCase")]
    SessionsStart {
        id: ReqId,
        workspace_id: Option<String>,
        project: Option<String>,
        branch: Option<String>,
        prompt: String,
        harness: Option<String>,
        thread: Option<String>,
        cwd: Option<String>,
        model: Option<String>,
        effort: Option<String>,
        permission_mode: Option<String>,
        access: Option<AccessChoice>,
        context_window: Option<String>,
        fast: Option<bool>,
        started_by: Option<SessionOrigin>,
        request_id: Option<String>,
        answer_held: Option<bool>,
        followed: Option<bool>,
        attempt: Option<String>,
        notify: Option<Vec<String>>,
        turn_token: Option<String>,
        title: Option<String>,
        resume: Option<ResumeAsk>,
        replaces: Option<String>,
        fork: Option<ForkSource>,
        attachments: Option<Vec<Attachment>>,
    },
    #[serde(rename = "harnesses.list", rename_all = "camelCase")]
    HarnessesList { id: ReqId, workspace_id: Option<String>, project: Option<String>, computer: Option<String> },
    #[serde(rename = "sessions.list", rename_all = "camelCase")]
    SessionsList { id: ReqId, workspace_id: Option<String> },
    #[serde(rename = "sessions.history", rename_all = "camelCase")]
    SessionsHistory {
        id: ReqId,
        workspace_id: String,
        thread_id: Option<String>,
        before: Option<i64>,
        limit: Option<i64>,
        whole: Option<True>,
    },
    #[serde(rename = "sessions.head", rename_all = "camelCase")]
    SessionsHead { id: ReqId, thread_id: String, after_read: Option<True> },
    #[serde(rename = "sessions.attachment", rename_all = "camelCase")]
    SessionsAttachment { id: ReqId, workspace_id: String, thread_id: String, request_id: String, index: i64 },
    #[serde(rename = "sessions.interrupt", rename_all = "camelCase")]
    SessionsInterrupt { id: ReqId, session_id: String, task: Option<String> },
    #[serde(rename = "sessions.steer", rename_all = "camelCase")]
    SessionsSteer { id: ReqId, session_id: String, prompt: String, request_id: Option<String>, attachments: Option<Vec<Attachment>> },
    #[serde(rename = "sessions.answer", rename_all = "camelCase")]
    SessionsAnswer { id: ReqId, session_id: String, ask_id: String, option_id: String, reason: Option<String> },
    #[serde(rename = "sessions.access", rename_all = "camelCase")]
    SessionsAccess { id: ReqId, session_id: String, permission_mode: String },
    #[serde(rename = "sessions.rename", rename_all = "camelCase")]
    SessionsRename { id: ReqId, session_id: String, title: String },
    #[serde(rename = "sessions.forget", rename_all = "camelCase")]
    SessionsForget { id: ReqId, thread_id: String, check: Option<bool> },
    #[serde(rename = "sessions.delete", rename_all = "camelCase")]
    SessionsDelete { id: ReqId, thread_id: String, processes: Option<ProcessesAsk>, check: Option<bool> },
    #[serde(rename = "sessions.read", rename_all = "camelCase")]
    SessionsRead { id: ReqId, thread_id: String },
    #[serde(rename = "sessions.replaceable", rename_all = "camelCase")]
    SessionsReplaceable { id: ReqId, thread_id: String },
    #[serde(rename = "sessions.warm", rename_all = "camelCase")]
    SessionsWarm {
        id: ReqId,
        workspace_id: Option<String>,
        project: Option<String>,
        cwd: Option<String>,
        harness: Option<String>,
        model: Option<String>,
        effort: Option<String>,
        permission_mode: Option<String>,
        access: Option<AccessChoice>,
        context_window: Option<String>,
        fast: Option<bool>,
    },
    #[serde(rename = "sessions.settle", rename_all = "camelCase")]
    SessionsSettle { id: ReqId, thread_ids: Vec<String>, finished: Option<bool>, processes: Option<ProcessesAsk> },
    #[serde(rename = "sessions.mark", rename_all = "camelCase")]
    SessionsMark { id: ReqId, thread_ids: Vec<String>, marks: ThreadMarks },
    #[serde(rename = "sessions.restore", rename_all = "camelCase")]
    SessionsRestore { id: ReqId, thread_ids: Vec<String> },
    #[serde(rename = "sessions.search", rename_all = "camelCase")]
    SessionsSearch { id: ReqId, query: String },
    #[serde(rename = "sessions.aside", rename_all = "camelCase")]
    SessionsAside { id: ReqId, session_id: String, question: String, ask_id: Option<String> },
    #[serde(rename = "sessions.run", rename_all = "camelCase")]
    SessionsRun {
        id: ReqId,
        run_id: String,
        block: String,
        command: String,
        state: RunState,
        pty_id: Option<String>,
        exit_code: Option<i64>,
        signal: Option<i64>,
        output: Option<String>,
        thread_id: String,
        turn_id: String,
    },
    #[serde(rename = "sessions.rewind", rename_all = "camelCase")]
    SessionsRewind { id: ReqId, thread_id: String, turn_id: Option<String>, files: Option<bool>, undo: Option<bool> },
    #[serde(rename = "golden.get", rename_all = "camelCase")]
    GoldenGet { id: ReqId, name: String },
    #[serde(rename = "capabilities.get", rename_all = "camelCase")]
    CapabilitiesGet { id: ReqId },
    #[serde(rename = "golden.prepare", rename_all = "camelCase")]
    GoldenPrepare { id: ReqId, name: String, kind: Option<MachineKind> },
    #[serde(rename = "golden.seal", rename_all = "camelCase")]
    GoldenSeal { id: ReqId, builder_id: String },
    #[serde(rename = "snapshots.list", rename_all = "camelCase")]
    SnapshotsList { id: ReqId, name: Option<String> },
    #[serde(rename = "snapshots.storage", rename_all = "camelCase")]
    SnapshotsStorage { id: ReqId },
    #[serde(rename = "cost.history", rename_all = "camelCase")]
    CostHistory { id: ReqId, workspace_id: String },
    #[serde(rename = "cost.spend", rename_all = "camelCase")]
    CostSpend { id: ReqId },
    #[serde(rename = "usage.used", rename_all = "camelCase")]
    UsageUsed { id: ReqId, range: UsageRange, split: UsageSplit, outside: Option<bool> },
    #[serde(rename = "usage.accounts", rename_all = "camelCase")]
    UsageAccounts { id: ReqId, fresh: Option<bool> },
    #[serde(rename = "usage.reset", rename_all = "camelCase")]
    UsageReset { id: ReqId, account: String, credit_id: Option<String>, on: Option<String> },
    #[serde(rename = "places.readings", rename_all = "camelCase")]
    PlacesReadings { id: ReqId, place_id: Option<String>, workspace_id: Option<String>, range: ReadingsRange },
    #[serde(rename = "snapshots.rollback", rename_all = "camelCase")]
    SnapshotsRollback { id: ReqId, version: JsNumber, name: Option<String> },
    #[serde(rename = "workspaces.portReach", rename_all = "camelCase")]
    WorkspacesPortReach { id: ReqId, workspace_id: String, port: i64 },
    #[serde(rename = "workspaces.portProbe", rename_all = "camelCase")]
    WorkspacesPortProbe { id: ReqId, workspace_id: String, port: i64 },
    #[serde(rename = "workspaces.rebuild", rename_all = "camelCase")]
    WorkspacesRebuild { id: ReqId, workspace_id: String },
    #[serde(rename = "forwards.list", rename_all = "camelCase")]
    ForwardsList { id: ReqId },
    #[serde(rename = "forwards.stop", rename_all = "camelCase")]
    ForwardsStop { id: ReqId, workspace_id: String, port: RelayPort },
    #[serde(rename = "workspaces.exec", rename_all = "camelCase")]
    WorkspacesExec { id: ReqId, workspace_id: String, argv: Vec<String>, cwd: Option<String> },
    #[serde(rename = "host.folders", rename_all = "camelCase")]
    HostFolders { id: ReqId, dir: Option<String>, hidden: Option<bool>, repos: Option<bool>, on: Option<String> },
    #[serde(rename = "host.terminalConfig", rename_all = "camelCase")]
    HostTerminalConfig { id: ReqId, scheme: Option<TerminalScheme> },
    #[serde(rename = "editor.list", rename_all = "camelCase")]
    EditorList { id: ReqId },
    #[serde(rename = "editor.open", rename_all = "camelCase")]
    EditorOpen { id: ReqId, workspace_id: String, path: String, line: Option<i64>, editor: Option<EditorId> },
    #[serde(rename = "ssh.port", rename_all = "camelCase")]
    SshPort { id: ReqId, workspace_id: String },
    #[serde(rename = "ssh.include", rename_all = "camelCase")]
    SshInclude { id: ReqId, on: Option<bool> },
    #[serde(rename = "agents.read", rename_all = "camelCase")]
    AgentsRead { id: ReqId, target: AgentsTarget },
    #[serde(rename = "servers.tools", rename_all = "camelCase")]
    ServersTools { id: ReqId, target: AgentsTarget, agent: String, name: String, refresh: Option<bool> },
    #[serde(rename = "servers.icon", rename_all = "camelCase")]
    ServersIcon { id: ReqId, host: String, refresh: Option<bool> },
    #[serde(rename = "agents.signIn", rename_all = "camelCase")]
    AgentsSignIn { id: ReqId, target: AgentsTarget, agent: String, terminal: Option<bool> },
    #[serde(rename = "servers.signIn", rename_all = "camelCase")]
    ServersSignIn { id: ReqId, target: AgentsTarget, agent: String, name: String, scope: Option<McpScope>, project: Option<String> },
    #[serde(rename = "agents.signInCode", rename_all = "camelCase")]
    AgentsSignInCode { id: ReqId, sign_in_id: String, code: String },
    #[serde(rename = "agents.signIns", rename_all = "camelCase")]
    AgentsSignIns { id: ReqId },
    #[serde(rename = "agents.signInStop", rename_all = "camelCase")]
    AgentsSignInStop { id: ReqId, sign_in_id: String },
    #[serde(rename = "agents.signInLine", rename_all = "camelCase")]
    AgentsSignInLine { id: ReqId, target: AgentsTarget, agent: String, name: Option<String> },
    #[serde(rename = "agents.key", rename_all = "camelCase")]
    AgentsKey { id: ReqId, agent: String, key: String, way: Option<AgentsKeyWay> },
    #[serde(rename = "agents.cloudSignIn", rename_all = "camelCase")]
    AgentsCloudSignIn { id: ReqId, agent: AgentsCloudSignInAgent },
    #[serde(rename = "providers.checkKey", rename_all = "camelCase")]
    ProvidersCheckKey { id: ReqId, provider: String, key: String },
    #[serde(rename = "broker.status", rename_all = "camelCase")]
    BrokerStatus { id: ReqId },
    #[serde(rename = "agents.addTools", rename_all = "camelCase")]
    AgentsAddTools { id: ReqId, target: AgentsTarget, agent: String },
    #[serde(rename = "agents.setup", rename_all = "camelCase")]
    AgentsSetup {
        id: ReqId,
        place_id: String,
        agent: String,
        on: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        program: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        config_dir: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        args: Option<Option<Vec<String>>>,
        env: Option<IndexMap<String, Option<String>>>,
    },
    #[serde(rename = "skills.search", rename_all = "camelCase")]
    SkillsSearch { id: ReqId, q: String, limit: Option<i64> },
    #[serde(rename = "skills.get", rename_all = "camelCase")]
    SkillsGet { id: ReqId, skill: String },
    #[serde(rename = "skills.preview", rename_all = "camelCase")]
    SkillsPreview { id: ReqId, target: AgentsTarget, name: String, project: Option<bool> },
    #[serde(rename = "skills.add", rename_all = "camelCase")]
    SkillsAdd { id: ReqId, target: AgentsTarget, skill: String, agents: Option<Vec<String>>, project: Option<bool> },
    #[serde(rename = "skills.remove", rename_all = "camelCase")]
    SkillsRemove { id: ReqId, target: AgentsTarget, name: String, project: Option<bool> },
    #[serde(rename = "skills.toggle", rename_all = "camelCase")]
    SkillsToggle { id: ReqId, target: AgentsTarget, name: String, project: Option<bool>, on: bool },
    #[serde(rename = "servers.add", rename_all = "camelCase")]
    ServersAdd {
        id: ReqId,
        target: AgentsTarget,
        agent: String,
        name: String,
        project: Option<bool>,
        command: Option<String>,
        args: Option<Vec<String>>,
        env: Option<IndexMap<String, String>>,
        url: Option<String>,
        headers: Option<IndexMap<String, String>>,
    },
    #[serde(rename = "servers.remove", rename_all = "camelCase")]
    ServersRemove { id: ReqId, target: AgentsTarget, agent: String, name: String, scope: Option<McpScope> },
    #[serde(rename = "servers.toggle", rename_all = "camelCase")]
    ServersToggle { id: ReqId, target: AgentsTarget, agent: String, name: String, scope: Option<McpScope>, on: bool },
    #[serde(rename = "plugins.toggle", rename_all = "camelCase")]
    PluginsToggle { id: ReqId, target: AgentsTarget, agent: String, plugin: String, on: bool },
    #[serde(rename = "init.get", rename_all = "camelCase")]
    InitGet { id: ReqId, on: Option<String> },
    #[serde(rename = "init.keys", rename_all = "camelCase")]
    InitKeys { id: ReqId, provider: Option<String>, key: Option<String>, rows: Option<IndexMap<String, String>> },
    #[serde(rename = "init.start", rename_all = "camelCase")]
    InitStart { id: ReqId, road: InitRoad, harness: Option<String>, on: Option<String> },
    #[serde(rename = "init.answer", rename_all = "camelCase")]
    InitAnswer { id: ReqId, screen: InitScreenId, ticks: Option<Vec<String>>, answers: Option<IndexMap<String, String>> },
    #[serde(rename = "init.step", rename_all = "camelCase")]
    InitStep { id: ReqId, at: i64 },
    #[serde(rename = "init.draft", rename_all = "camelCase")]
    InitDraft { id: ReqId, at: String, ticks: Option<Vec<String>>, answers: Option<IndexMap<String, String>> },
    #[serde(rename = "init.retry", rename_all = "camelCase")]
    InitRetry { id: ReqId, tool: String },
    #[serde(rename = "init.build", rename_all = "camelCase")]
    InitBuild {
        id: ReqId,
        first_workspace: Option<String>,
        import_folder: Option<String>,
        yes: Option<bool>,
        on: Option<String>,
        rebuild: Option<bool>,
    },
    #[serde(rename = "init.signInCode", rename_all = "camelCase")]
    InitSignInCode { id: ReqId, tool: String, code: String },
    #[serde(rename = "init.cancel", rename_all = "camelCase")]
    InitCancel { id: ReqId },
    #[serde(rename = "preferences.get", rename_all = "camelCase")]
    PreferencesGet { id: ReqId },
    #[serde(rename = "preferences.set", rename_all = "camelCase")]
    PreferencesSet { id: ReqId, patch: PreferencesPatch },
    #[serde(rename = "release.get", rename_all = "camelCase")]
    ReleaseGet { id: ReqId },
    #[serde(rename = "release.check", rename_all = "camelCase")]
    ReleaseCheck { id: ReqId, force: Option<bool> },
    #[serde(rename = "host.restart", rename_all = "camelCase")]
    HostRestart { id: ReqId },
    #[serde(rename = "projects.add", rename_all = "camelCase")]
    ProjectsAdd {
        id: ReqId,
        source: String,
        on: Option<String>,
        name: Option<String>,
        base: Option<String>,
        into: Option<String>,
        seed: Option<SeedChoice>,
    },
    #[serde(rename = "project.seed.plan", rename_all = "camelCase")]
    ProjectSeedPlan { id: ReqId, source: String, on: Option<String> },
    #[serde(rename = "projects.seed", rename_all = "camelCase")]
    ProjectsSeed { id: ReqId, project: String, keep: Option<Vec<String>>, cut: Option<Vec<String>>, none: Option<bool>, save: Option<bool> },
    #[serde(rename = "projects.list", rename_all = "camelCase")]
    ProjectsList { id: ReqId },
    #[serde(rename = "conversations.list", rename_all = "camelCase")]
    ConversationsList { id: ReqId, project: Option<String>, agent: Option<String> },
    #[serde(rename = "conversations.read", rename_all = "camelCase")]
    ConversationsRead { id: ReqId, project: Option<String>, agent: String, conversation: String, preview: Option<bool> },
    #[serde(rename = "projects.defaults", rename_all = "camelCase")]
    ProjectsDefaults { id: ReqId },
    #[serde(rename = "projects.resolve", rename_all = "camelCase")]
    ProjectsResolve { id: ReqId, r#ref: String },
    #[serde(rename = "projects.branch", rename_all = "camelCase")]
    ProjectsBranch { id: ReqId, project_id: String },
    #[serde(rename = "projects.remove", rename_all = "camelCase")]
    ProjectsRemove { id: ReqId, project_id: String, force: Option<bool>, check: Option<bool> },
    #[serde(rename = "projects.icon", rename_all = "camelCase")]
    ProjectsIcon {
        id: ReqId,
        project_id: String,
        #[serialize_always]
        png: Option<String>,
    },
    #[serde(rename = "projects.icons", rename_all = "camelCase")]
    ProjectsIcons { id: ReqId, hashes: Vec<ProjectIconHash> },
    #[serde(rename = "project.plan", rename_all = "camelCase")]
    ProjectPlan { id: ReqId, source: String },
    #[serde(rename = "project.import", rename_all = "camelCase")]
    ProjectImport {
        id: ReqId,
        workspace_id: String,
        source: String,
        dest: String,
        replace: Option<bool>,
        carry: Option<Vec<String>>,
        rewrite: Option<Vec<String>>,
        agents: Option<Vec<String>>,
    },
    #[serde(rename = "project.export", rename_all = "camelCase")]
    ProjectExport { id: ReqId, workspace_id: String, source: String, dest: String, replace: Option<bool>, agents: Option<Vec<String>> },
    #[serde(rename = "image.get", rename_all = "camelCase")]
    ImageGet { id: ReqId, name: Option<String> },
    #[serde(rename = "image.build", rename_all = "camelCase")]
    ImageBuild { id: ReqId, place: String, name: Option<String>, force: Option<bool> },
    #[serde(rename = "image.export", rename_all = "camelCase")]
    ImageExport { id: ReqId, dest: String, passphrase: String, name: Option<String> },
    #[serde(rename = "slates.get", rename_all = "camelCase")]
    SlatesGet { id: ReqId, thread_id: String },
    #[serde(rename = "slates.write", rename_all = "camelCase")]
    SlatesWrite {
        id: ReqId,
        thread_id: Option<String>,
        turn_token: Option<String>,
        text: Option<String>,
        document: Option<IndexMap<String, Value>>,
        check: Option<bool>,
        if_version: Option<i64>,
        values: Option<SlateWireValues>,
        press: Option<SlatesWritePress>,
    },
    #[serde(rename = "slates.state", rename_all = "camelCase")]
    SlatesState {
        id: ReqId,
        thread_id: Option<String>,
        turn_token: Option<String>,
        values: Option<SlateWireValues>,
        start: Option<Vec<String>>,
        if_version: Option<i64>,
    },
    #[serde(rename = "slates.read", rename_all = "camelCase")]
    SlatesRead {
        id: ReqId,
        thread_id: Option<String>,
        turn_token: Option<String>,
        values: Option<Vec<String>>,
        text: Option<bool>,
        sketch: Option<bool>,
        document: Option<bool>,
    },
    #[serde(rename = "slates.catalog", rename_all = "camelCase")]
    SlatesCatalog { id: ReqId, name: Option<String>, thread_id: Option<String>, turn_token: Option<String> },
    #[serde(rename = "slates.event", rename_all = "camelCase")]
    SlatesEvent {
        id: ReqId,
        thread_id: String,
        version: i64,
        piece: String,
        event: SlatesEventEvent,
        request_id: String,
        scope: Option<SlatesEventScope>,
        row_action: Option<i64>,
    },
    #[serde(rename = "slates.approve", rename_all = "camelCase")]
    SlatesApprove { id: ReqId, thread_id: String, key: String, scope: SlatesApproveScope },
    #[serde(rename = "slates.cancel", rename_all = "camelCase")]
    SlatesCancel { id: ReqId, thread_id: String, run: String },
    #[serde(rename = "slates.revoke", rename_all = "camelCase")]
    SlatesRevoke { id: ReqId, thread_id: String, key: String },
    #[serde(rename = "slates.shown", rename_all = "camelCase")]
    SlatesShown { id: ReqId, thread_id: String },
    #[serde(rename = "slates.subscribe", rename_all = "camelCase")]
    SlatesSubscribe { id: ReqId, thread_id: String, sources: Vec<String> },
    #[serde(rename = "slates.unsubscribe", rename_all = "camelCase")]
    SlatesUnsubscribe { id: ReqId, thread_id: String, sources: Vec<String> },
    #[serde(rename = "slates.resolve", rename_all = "camelCase")]
    SlatesResolve { id: ReqId, thread_id: String, paths: Vec<String> },
    #[serde(rename = "slates.image", rename_all = "camelCase")]
    SlatesImage { id: ReqId, thread_id: String, src: String, have: Option<String> },
}
/// One frame a client sends the host: the op and its params, and where the frame says it reached the host from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeRequest {
    #[serde(flatten)]
    pub op: RuntimeOp,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub origin: Option<WorkspaceOrigin>,
}

/// Reads one client frame as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of RUNTIME_OPS in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
/// `origin`, where present, is one of here, relayed, paired. The `id` is read as RequestId: a string or a number.
pub fn parse_runtime_request(frame: &Value) -> Result<RuntimeRequest, SchemaError> {
    let _ = frame;
    todo!()
}

impl RuntimeOp {
    /// The op word this variant is read from and written as: "places.list" for PlacesList.
    pub fn name(&self) -> &'static str {
        todo!()
    }

    /// The request id the frame carried, which the reply echoes.
    pub fn id(&self) -> &ReqId {
        todo!()
    }
}
