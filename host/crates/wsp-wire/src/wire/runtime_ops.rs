// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts lines 1-1029: the RuntimeOp union every client frame is read by,
//! the envelope beside it, and its reader. The caps of each variant are listed at the foot of runtime.rs.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::helpers::ReqId;
use crate::types::agents_report::{AgentsTarget, McpScope};
use crate::types::attachments::Attachment;
use crate::types::check::Zod;
use crate::types::conversations::ResumeAsk;
use crate::types::project_look::ProjectIconHash;
use crate::types::pull_request::{MergeMethod, PullRequestItem, ReactionContent};
use crate::types::recipe_file::RecipeFile;
use crate::types::slate_wire::SlateWireValues;
use crate::types::thread_defaults::AccessChoice;
use crate::types::usage::{ReadingsRange, UsageRange, UsageSplit};
use crate::types::workspace_look::{WorkspaceGlyph, WorkspaceTheme};
use crate::types::{parse, Check, JsNumber, SchemaError};
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
        threads: Option<JsInt>,
        machines: Option<JsInt>,
        spend_per_day_usd: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        nap_ms: Option<Option<JsInt>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        turn_limit_ms: Option<Option<JsInt>>,
        spawn: Option<PlacesSetSpawn>,
        size: Option<PlacesSetSize>,
        delete_after_ms: Option<JsInt>,
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
        ssh_port: Option<JsInt>,
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
    RecipesSave {
        id: ReqId,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        file: Option<Option<crate::types::kept::Kept>>,
        from: Option<String>,
    },
    #[serde(rename = "recipes.remove", rename_all = "camelCase")]
    RecipesRemove { id: ReqId, name: String, check: Option<bool> },
    #[serde(rename = "recipes.options", rename_all = "camelCase")]
    RecipesOptions { id: ReqId },
    #[serde(rename = "events.subscribe", rename_all = "camelCase")]
    EventsSubscribe { id: ReqId, after: Option<JsInt>, stream: Option<String>, creates: Option<bool> },
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
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
        envs: Option<IndexMap<String, String>>,
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
        labels: Option<IndexMap<String, String>>,
        agents: Option<WorkspacesCreateAgents>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
        idle_window_ms: Option<Option<JsNumber>>,
        parent: Option<String>,
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
    WorkspacesPullRequestReply { id: ReqId, workspace_id: String, reply_to: Option<JsInt>, thread_id: Option<String>, body: String },
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
    ActivityEnd { id: ReqId, computer: Option<String>, pid: JsInt, started_at: JsNumber },
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
        before: Option<JsInt>,
        limit: Option<JsInt>,
        whole: Option<True>,
    },
    #[serde(rename = "sessions.head", rename_all = "camelCase")]
    SessionsHead { id: ReqId, thread_id: String, after_read: Option<True> },
    #[serde(rename = "sessions.attachment", rename_all = "camelCase")]
    SessionsAttachment { id: ReqId, workspace_id: String, thread_id: String, request_id: String, index: JsInt },
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
        exit_code: Option<JsInt>,
        signal: Option<JsInt>,
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
    WorkspacesPortReach { id: ReqId, workspace_id: String, port: JsInt },
    #[serde(rename = "workspaces.portProbe", rename_all = "camelCase")]
    WorkspacesPortProbe { id: ReqId, workspace_id: String, port: JsInt },
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
    EditorOpen { id: ReqId, workspace_id: String, path: String, line: Option<JsInt>, editor: Option<EditorId> },
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
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
        env: Option<IndexMap<String, Option<String>>>,
    },
    #[serde(rename = "skills.search", rename_all = "camelCase")]
    SkillsSearch { id: ReqId, q: String, limit: Option<JsInt> },
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
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
        env: Option<IndexMap<String, String>>,
        url: Option<String>,
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
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
    InitKeys { id: ReqId, provider: Option<String>, key: Option<String> },
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
    #[serde(rename = "slates.get", rename_all = "camelCase")]
    SlatesGet { id: ReqId, thread_id: String },
    #[serde(rename = "slates.write", rename_all = "camelCase")]
    SlatesWrite {
        id: ReqId,
        thread_id: Option<String>,
        turn_token: Option<String>,
        text: Option<String>,
        #[serde(default, serialize_with = "crate::types::check::js_record_opt", deserialize_with = "crate::types::kept::record_opt")]
        document: Option<IndexMap<String, crate::types::kept::Kept>>,
        check: Option<bool>,
        if_version: Option<JsInt>,
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
        if_version: Option<JsInt>,
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
        version: JsInt,
        piece: String,
        event: SlatesEventEvent,
        request_id: String,
        scope: Option<SlatesEventScope>,
        row_action: Option<JsInt>,
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
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeRequest {
    pub op: RuntimeOp,
    pub origin: Option<WorkspaceOrigin>,
}

impl Serialize for RuntimeRequest {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Framed<'a> {
            #[serde(flatten)]
            op: &'a RuntimeOp,
            #[serde(skip_serializing_if = "Option::is_none")]
            origin: Option<&'a WorkspaceOrigin>,
        }
        Framed { op: &self.op, origin: self.origin.as_ref() }.serialize(s)
    }
}

/// Reads what `RuntimeRequest::pass` has found clean: the op, and `origin` where the frame has one.
impl<'de> Deserialize<'de> for RuntimeRequest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let frame = Value::deserialize(d)?;
        let op = serde_json::from_value(frame.clone()).map_err(D::Error::custom)?;
        let origin = frame.get("origin").map(|origin| serde_json::from_value(origin.clone())).transpose().map_err(D::Error::custom)?;
        Ok(RuntimeRequest { op, origin })
    }
}

/// A rule on an `.optional()` key: it runs on the value when the key has one.
pub(super) fn opt_rule<T>(x: &Option<T>, rule: impl FnOnce(&T) -> Result<(), SchemaError>) -> Result<(), SchemaError> {
    x.as_ref().map_or(Ok(()), rule)
}

/// A rule on an `.optional().nullable()` key: it runs on a value, not on `null`.
pub(super) fn dbl_rule<T>(x: &Option<Option<T>>, rule: impl FnOnce(&T) -> Result<(), SchemaError>) -> Result<(), SchemaError> {
    x.as_ref().and_then(Option::as_ref).map_or(Ok(()), rule)
}

/// The issue zod's `EnvName` gives a variable name the shell would not take, at the key's own path below the record.
pub(super) fn env_name_issue(key: &str) -> SchemaError {
    SchemaError {
        code: "invalid_string".into(),
        path: vec![Value::from(key)],
        message: "a variable's name is letters, digits and _, not starting with a digit".into(),
        detail: Box::new(IndexMap::from([("validation".to_string(), Value::from("regex"))])),
    }
}

/// `z.record(EnvName, z.string().nullable()).optional()`: zod checks each entry's name, then its value, in the order the
/// frame holds them, so a second bad name is a second issue.
pub(super) fn env_key(obj: &Map<String, Value>, out: &mut Vec<SchemaError>) {
    let Some(Value::Object(vars)) = obj.get("env") else {
        return crate::types::keys::opt_key::<IndexMap<String, Option<String>>>(obj, "env", out);
    };
    for (name, value) in crate::types::kept::js_order(vars.iter().map(|(k, v)| (k.as_str(), v))) {
        if !crate::types::check::env_name(name) {
            out.push(env_name_issue(name).under("env"));
        }
        out.extend(crate::types::check::issues_of::<Option<String>>(value).into_iter().map(|e| e.under(name).under("env")));
    }
}

/// The keys of the ops whose object zod reads strict: a merge keeps the strictness of the schema merged in, so these ops
/// refuse a key they do not name, `origin` among them.
pub(super) fn strict_keys(word: &str) -> Option<&'static [&'static str]> {
    match word {
        "agents.setup" => Some(&["id", "op", "placeId", "agent", "on", "program", "configDir", "args", "env"]),
        _ => None,
    }
}

/// Reads one client frame as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of RUNTIME_OPS in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's fields in the
/// schema's order, then its Check, then `origin`. Unknown keys are dropped.
/// `origin`, where present, is one of here, relayed, paired. The `id` is read as RequestId: a string or a number.
pub fn parse_runtime_request(frame: &Value) -> Result<RuntimeRequest, SchemaError> {
    parse::<RuntimeRequest>(frame)
}

impl RuntimeOp {
    /// The op word this variant is read from and written as: "places.list" for PlacesList.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Auth { .. } => "auth",
            Self::TicketIssue { .. } => "ticket.issue",
            Self::PairIssue { .. } => "pair.issue",
            Self::PairRedeem { .. } => "pair.redeem",
            Self::SealOpen(_) => "seal.open",
            Self::DeviceAuth(_) => "device.auth",
            Self::AccountGet { .. } => "account.get",
            Self::DevicesList { .. } => "devices.list",
            Self::DevicesRevoke { .. } => "devices.revoke",
            Self::PlaceJoin(_) => "place.join",
            Self::PlaceAuth(_) => "place.auth",
            Self::PlaceProve(_) => "place.prove",
            Self::PlacesList { .. } => "places.list",
            Self::PlacesUpdate { .. } => "places.update",
            Self::PlacesHolds { .. } => "places.holds",
            Self::PlacesRemove { .. } => "places.remove",
            Self::PlacesDoctor { .. } => "places.doctor",
            Self::PlacesDial { .. } => "places.dial",
            Self::PlacesSet { .. } => "places.set",
            Self::PlacesFollow { .. } => "places.follow",
            Self::PlacesSkip { .. } => "places.skip",
            Self::PlacesSetupLog { .. } => "places.setupLog",
            Self::PlacesEstimate { .. } => "places.estimate",
            Self::PlacesLoginLanded { .. } => "places.loginLanded",
            Self::PlacesDoor { .. } => "places.door",
            Self::PlacesMint { .. } => "places.mint",
            Self::PlacesSshHosts { .. } => "places.sshHosts",
            Self::PlacesAdd { .. } => "places.add",
            Self::PlacesSetup { .. } => "places.setup",
            Self::PlacesChoose { .. } => "places.choose",
            Self::RecipesList { .. } => "recipes.list",
            Self::RecipesGet { .. } => "recipes.get",
            Self::RecipesSave { .. } => "recipes.save",
            Self::RecipesRemove { .. } => "recipes.remove",
            Self::RecipesOptions { .. } => "recipes.options",
            Self::EventsSubscribe { .. } => "events.subscribe",
            Self::StatusSubscribe { .. } => "status.subscribe",
            Self::StatusList { .. } => "status.list",
            Self::WorkspacesCreate { .. } => "workspaces.create",
            Self::WorkspacesLanding { .. } => "workspaces.landing",
            Self::WorkspacesList { .. } => "workspaces.list",
            Self::WorkspacesResolve { .. } => "workspaces.resolve",
            Self::WorkspacesGet { .. } => "workspaces.get",
            Self::WorkspacesNap { .. } => "workspaces.nap",
            Self::WorkspacesResize { .. } => "workspaces.resize",
            Self::WorkspacesWake { .. } => "workspaces.wake",
            Self::WorkspacesRestartDaemon { .. } => "workspaces.restartDaemon",
            Self::WorkspacesStopWake { .. } => "workspaces.stopWake",
            Self::WorkspacesUpgrade { .. } => "workspaces.upgrade",
            Self::WorkspacesRename { .. } => "workspaces.rename",
            Self::WorkspacesLook { .. } => "workspaces.look",
            Self::WorkspacesBringBack { .. } => "workspaces.bringBack",
            Self::WorktreeMake { .. } => "worktree.make",
            Self::FolderMake { .. } => "folder.make",
            Self::WorktreeRemove { .. } => "worktree.remove",
            Self::WorkspacesCheckout { .. } => "workspaces.checkout",
            Self::WorkspacesDiscard { .. } => "workspaces.discard",
            Self::WorkspacesCommit { .. } => "workspaces.commit",
            Self::WorkspacesCommitDraft { .. } => "workspaces.commitDraft",
            Self::WorkspacesViewed { .. } => "workspaces.viewed",
            Self::WorkspacesPullRequestView { .. } => "workspaces.pullRequestView",
            Self::WorkspacesPullRequestDiff { .. } => "workspaces.pullRequestDiff",
            Self::WorkspacesPullRequestSend { .. } => "workspaces.pullRequestSend",
            Self::WorkspacesPullRequestReply { .. } => "workspaces.pullRequestReply",
            Self::WorkspacesPullRequestResolve { .. } => "workspaces.pullRequestResolve",
            Self::WorkspacesPullRequestReact { .. } => "workspaces.pullRequestReact",
            Self::WorkspacesFix { .. } => "workspaces.fix",
            Self::WorkspacesMerge { .. } => "workspaces.merge",
            Self::WorkspacesStart { .. } => "workspaces.start",
            Self::WorkspacesReview { .. } => "workspaces.review",
            Self::WorkspacesReviewDraft { .. } => "workspaces.reviewDraft",
            Self::WorkspacesReviewPost { .. } => "workspaces.reviewPost",
            Self::WorkspacesUpdate { .. } => "workspaces.update",
            Self::WorkspacesMergeIn { .. } => "workspaces.mergeIn",
            Self::WorkspacesDelete { .. } => "workspaces.delete",
            Self::WorkspacesForget { .. } => "workspaces.forget",
            Self::WorkspacesTouch { .. } => "workspaces.touch",
            Self::DaemonOpen { .. } => "daemon.open",
            Self::SysSubscribe { .. } => "sys.subscribe",
            Self::SysUnsubscribe { .. } => "sys.unsubscribe",
            Self::AppProcsWatch { .. } => "appProcs.watch",
            Self::AppProcsUnwatch { .. } => "appProcs.unwatch",
            Self::AppProcsOffer { .. } => "appProcs.offer",
            Self::AppProcsSet { .. } => "appProcs.set",
            Self::ActivityRead { .. } => "activity.read",
            Self::ActivityEnd { .. } => "activity.end",
            Self::DaemonSend { .. } => "daemon.send",
            Self::DaemonClose { .. } => "daemon.close",
            Self::SessionsStart { .. } => "sessions.start",
            Self::HarnessesList { .. } => "harnesses.list",
            Self::SessionsList { .. } => "sessions.list",
            Self::SessionsHistory { .. } => "sessions.history",
            Self::SessionsHead { .. } => "sessions.head",
            Self::SessionsAttachment { .. } => "sessions.attachment",
            Self::SessionsInterrupt { .. } => "sessions.interrupt",
            Self::SessionsSteer { .. } => "sessions.steer",
            Self::SessionsAnswer { .. } => "sessions.answer",
            Self::SessionsAccess { .. } => "sessions.access",
            Self::SessionsRename { .. } => "sessions.rename",
            Self::SessionsForget { .. } => "sessions.forget",
            Self::SessionsDelete { .. } => "sessions.delete",
            Self::SessionsRead { .. } => "sessions.read",
            Self::SessionsReplaceable { .. } => "sessions.replaceable",
            Self::SessionsWarm { .. } => "sessions.warm",
            Self::SessionsSettle { .. } => "sessions.settle",
            Self::SessionsMark { .. } => "sessions.mark",
            Self::SessionsRestore { .. } => "sessions.restore",
            Self::SessionsSearch { .. } => "sessions.search",
            Self::SessionsAside { .. } => "sessions.aside",
            Self::SessionsRun { .. } => "sessions.run",
            Self::SessionsRewind { .. } => "sessions.rewind",
            Self::GoldenGet { .. } => "golden.get",
            Self::CapabilitiesGet { .. } => "capabilities.get",
            Self::GoldenPrepare { .. } => "golden.prepare",
            Self::GoldenSeal { .. } => "golden.seal",
            Self::SnapshotsList { .. } => "snapshots.list",
            Self::SnapshotsStorage { .. } => "snapshots.storage",
            Self::CostHistory { .. } => "cost.history",
            Self::CostSpend { .. } => "cost.spend",
            Self::UsageUsed { .. } => "usage.used",
            Self::UsageAccounts { .. } => "usage.accounts",
            Self::UsageReset { .. } => "usage.reset",
            Self::PlacesReadings { .. } => "places.readings",
            Self::SnapshotsRollback { .. } => "snapshots.rollback",
            Self::WorkspacesPortReach { .. } => "workspaces.portReach",
            Self::WorkspacesPortProbe { .. } => "workspaces.portProbe",
            Self::WorkspacesRebuild { .. } => "workspaces.rebuild",
            Self::ForwardsList { .. } => "forwards.list",
            Self::ForwardsStop { .. } => "forwards.stop",
            Self::WorkspacesExec { .. } => "workspaces.exec",
            Self::HostFolders { .. } => "host.folders",
            Self::HostTerminalConfig { .. } => "host.terminalConfig",
            Self::EditorList { .. } => "editor.list",
            Self::EditorOpen { .. } => "editor.open",
            Self::SshPort { .. } => "ssh.port",
            Self::SshInclude { .. } => "ssh.include",
            Self::AgentsRead { .. } => "agents.read",
            Self::ServersTools { .. } => "servers.tools",
            Self::ServersIcon { .. } => "servers.icon",
            Self::AgentsSignIn { .. } => "agents.signIn",
            Self::ServersSignIn { .. } => "servers.signIn",
            Self::AgentsSignInCode { .. } => "agents.signInCode",
            Self::AgentsSignIns { .. } => "agents.signIns",
            Self::AgentsSignInStop { .. } => "agents.signInStop",
            Self::AgentsSignInLine { .. } => "agents.signInLine",
            Self::AgentsKey { .. } => "agents.key",
            Self::AgentsCloudSignIn { .. } => "agents.cloudSignIn",
            Self::ProvidersCheckKey { .. } => "providers.checkKey",
            Self::BrokerStatus { .. } => "broker.status",
            Self::AgentsAddTools { .. } => "agents.addTools",
            Self::AgentsSetup { .. } => "agents.setup",
            Self::SkillsSearch { .. } => "skills.search",
            Self::SkillsGet { .. } => "skills.get",
            Self::SkillsPreview { .. } => "skills.preview",
            Self::SkillsAdd { .. } => "skills.add",
            Self::SkillsRemove { .. } => "skills.remove",
            Self::SkillsToggle { .. } => "skills.toggle",
            Self::ServersAdd { .. } => "servers.add",
            Self::ServersRemove { .. } => "servers.remove",
            Self::ServersToggle { .. } => "servers.toggle",
            Self::PluginsToggle { .. } => "plugins.toggle",
            Self::InitGet { .. } => "init.get",
            Self::InitKeys { .. } => "init.keys",
            Self::PreferencesGet { .. } => "preferences.get",
            Self::PreferencesSet { .. } => "preferences.set",
            Self::ReleaseGet { .. } => "release.get",
            Self::ReleaseCheck { .. } => "release.check",
            Self::HostRestart { .. } => "host.restart",
            Self::ProjectsAdd { .. } => "projects.add",
            Self::ProjectSeedPlan { .. } => "project.seed.plan",
            Self::ProjectsSeed { .. } => "projects.seed",
            Self::ProjectsList { .. } => "projects.list",
            Self::ConversationsList { .. } => "conversations.list",
            Self::ConversationsRead { .. } => "conversations.read",
            Self::ProjectsDefaults { .. } => "projects.defaults",
            Self::ProjectsResolve { .. } => "projects.resolve",
            Self::ProjectsBranch { .. } => "projects.branch",
            Self::ProjectsRemove { .. } => "projects.remove",
            Self::ProjectsIcon { .. } => "projects.icon",
            Self::ProjectsIcons { .. } => "projects.icons",
            Self::ProjectPlan { .. } => "project.plan",
            Self::ProjectImport { .. } => "project.import",
            Self::ProjectExport { .. } => "project.export",
            Self::ImageGet { .. } => "image.get",
            Self::ImageBuild { .. } => "image.build",
            Self::SlatesGet { .. } => "slates.get",
            Self::SlatesWrite { .. } => "slates.write",
            Self::SlatesState { .. } => "slates.state",
            Self::SlatesRead { .. } => "slates.read",
            Self::SlatesCatalog { .. } => "slates.catalog",
            Self::SlatesEvent { .. } => "slates.event",
            Self::SlatesApprove { .. } => "slates.approve",
            Self::SlatesCancel { .. } => "slates.cancel",
            Self::SlatesRevoke { .. } => "slates.revoke",
            Self::SlatesShown { .. } => "slates.shown",
            Self::SlatesSubscribe { .. } => "slates.subscribe",
            Self::SlatesUnsubscribe { .. } => "slates.unsubscribe",
            Self::SlatesResolve { .. } => "slates.resolve",
            Self::SlatesImage { .. } => "slates.image",
        }
    }

    /// The request id the frame carried, which the reply echoes.
    pub fn id(&self) -> &ReqId {
        match self {
            Self::Auth { id, .. } => id,
            Self::TicketIssue { id, .. } => id,
            Self::PairIssue { id, .. } => id,
            Self::PairRedeem { id, .. } => id,
            Self::SealOpen(r) => &r.id,
            Self::DeviceAuth(r) => &r.id,
            Self::AccountGet { id, .. } => id,
            Self::DevicesList { id, .. } => id,
            Self::DevicesRevoke { id, .. } => id,
            Self::PlaceJoin(r) => &r.id,
            Self::PlaceAuth(r) => &r.id,
            Self::PlaceProve(r) => &r.id,
            Self::PlacesList { id, .. } => id,
            Self::PlacesUpdate { id, .. } => id,
            Self::PlacesHolds { id, .. } => id,
            Self::PlacesRemove { id, .. } => id,
            Self::PlacesDoctor { id, .. } => id,
            Self::PlacesDial { id, .. } => id,
            Self::PlacesSet { id, .. } => id,
            Self::PlacesFollow { id, .. } => id,
            Self::PlacesSkip { id, .. } => id,
            Self::PlacesSetupLog { id, .. } => id,
            Self::PlacesEstimate { id, .. } => id,
            Self::PlacesLoginLanded { id, .. } => id,
            Self::PlacesDoor { id, .. } => id,
            Self::PlacesMint { id, .. } => id,
            Self::PlacesSshHosts { id, .. } => id,
            Self::PlacesAdd { id, .. } => id,
            Self::PlacesSetup { id, .. } => id,
            Self::PlacesChoose { id, .. } => id,
            Self::RecipesList { id, .. } => id,
            Self::RecipesGet { id, .. } => id,
            Self::RecipesSave { id, .. } => id,
            Self::RecipesRemove { id, .. } => id,
            Self::RecipesOptions { id, .. } => id,
            Self::EventsSubscribe { id, .. } => id,
            Self::StatusSubscribe { id, .. } => id,
            Self::StatusList { id, .. } => id,
            Self::WorkspacesCreate { id, .. } => id,
            Self::WorkspacesLanding { id, .. } => id,
            Self::WorkspacesList { id, .. } => id,
            Self::WorkspacesResolve { id, .. } => id,
            Self::WorkspacesGet { id, .. } => id,
            Self::WorkspacesNap { id, .. } => id,
            Self::WorkspacesResize { id, .. } => id,
            Self::WorkspacesWake { id, .. } => id,
            Self::WorkspacesRestartDaemon { id, .. } => id,
            Self::WorkspacesStopWake { id, .. } => id,
            Self::WorkspacesUpgrade { id, .. } => id,
            Self::WorkspacesRename { id, .. } => id,
            Self::WorkspacesLook { id, .. } => id,
            Self::WorkspacesBringBack { id, .. } => id,
            Self::WorktreeMake { id, .. } => id,
            Self::FolderMake { id, .. } => id,
            Self::WorktreeRemove { id, .. } => id,
            Self::WorkspacesCheckout { id, .. } => id,
            Self::WorkspacesDiscard { id, .. } => id,
            Self::WorkspacesCommit { id, .. } => id,
            Self::WorkspacesCommitDraft { id, .. } => id,
            Self::WorkspacesViewed { id, .. } => id,
            Self::WorkspacesPullRequestView { id, .. } => id,
            Self::WorkspacesPullRequestDiff { id, .. } => id,
            Self::WorkspacesPullRequestSend { id, .. } => id,
            Self::WorkspacesPullRequestReply { id, .. } => id,
            Self::WorkspacesPullRequestResolve { id, .. } => id,
            Self::WorkspacesPullRequestReact { id, .. } => id,
            Self::WorkspacesFix { id, .. } => id,
            Self::WorkspacesMerge { id, .. } => id,
            Self::WorkspacesStart { id, .. } => id,
            Self::WorkspacesReview { id, .. } => id,
            Self::WorkspacesReviewDraft { id, .. } => id,
            Self::WorkspacesReviewPost { id, .. } => id,
            Self::WorkspacesUpdate { id, .. } => id,
            Self::WorkspacesMergeIn { id, .. } => id,
            Self::WorkspacesDelete { id, .. } => id,
            Self::WorkspacesForget { id, .. } => id,
            Self::WorkspacesTouch { id, .. } => id,
            Self::DaemonOpen { id, .. } => id,
            Self::SysSubscribe { id, .. } => id,
            Self::SysUnsubscribe { id, .. } => id,
            Self::AppProcsWatch { id, .. } => id,
            Self::AppProcsUnwatch { id, .. } => id,
            Self::AppProcsOffer { id, .. } => id,
            Self::AppProcsSet { id, .. } => id,
            Self::ActivityRead { id, .. } => id,
            Self::ActivityEnd { id, .. } => id,
            Self::DaemonSend { id, .. } => id,
            Self::DaemonClose { id, .. } => id,
            Self::SessionsStart { id, .. } => id,
            Self::HarnessesList { id, .. } => id,
            Self::SessionsList { id, .. } => id,
            Self::SessionsHistory { id, .. } => id,
            Self::SessionsHead { id, .. } => id,
            Self::SessionsAttachment { id, .. } => id,
            Self::SessionsInterrupt { id, .. } => id,
            Self::SessionsSteer { id, .. } => id,
            Self::SessionsAnswer { id, .. } => id,
            Self::SessionsAccess { id, .. } => id,
            Self::SessionsRename { id, .. } => id,
            Self::SessionsForget { id, .. } => id,
            Self::SessionsDelete { id, .. } => id,
            Self::SessionsRead { id, .. } => id,
            Self::SessionsReplaceable { id, .. } => id,
            Self::SessionsWarm { id, .. } => id,
            Self::SessionsSettle { id, .. } => id,
            Self::SessionsMark { id, .. } => id,
            Self::SessionsRestore { id, .. } => id,
            Self::SessionsSearch { id, .. } => id,
            Self::SessionsAside { id, .. } => id,
            Self::SessionsRun { id, .. } => id,
            Self::SessionsRewind { id, .. } => id,
            Self::GoldenGet { id, .. } => id,
            Self::CapabilitiesGet { id, .. } => id,
            Self::GoldenPrepare { id, .. } => id,
            Self::GoldenSeal { id, .. } => id,
            Self::SnapshotsList { id, .. } => id,
            Self::SnapshotsStorage { id, .. } => id,
            Self::CostHistory { id, .. } => id,
            Self::CostSpend { id, .. } => id,
            Self::UsageUsed { id, .. } => id,
            Self::UsageAccounts { id, .. } => id,
            Self::UsageReset { id, .. } => id,
            Self::PlacesReadings { id, .. } => id,
            Self::SnapshotsRollback { id, .. } => id,
            Self::WorkspacesPortReach { id, .. } => id,
            Self::WorkspacesPortProbe { id, .. } => id,
            Self::WorkspacesRebuild { id, .. } => id,
            Self::ForwardsList { id, .. } => id,
            Self::ForwardsStop { id, .. } => id,
            Self::WorkspacesExec { id, .. } => id,
            Self::HostFolders { id, .. } => id,
            Self::HostTerminalConfig { id, .. } => id,
            Self::EditorList { id, .. } => id,
            Self::EditorOpen { id, .. } => id,
            Self::SshPort { id, .. } => id,
            Self::SshInclude { id, .. } => id,
            Self::AgentsRead { id, .. } => id,
            Self::ServersTools { id, .. } => id,
            Self::ServersIcon { id, .. } => id,
            Self::AgentsSignIn { id, .. } => id,
            Self::ServersSignIn { id, .. } => id,
            Self::AgentsSignInCode { id, .. } => id,
            Self::AgentsSignIns { id, .. } => id,
            Self::AgentsSignInStop { id, .. } => id,
            Self::AgentsSignInLine { id, .. } => id,
            Self::AgentsKey { id, .. } => id,
            Self::AgentsCloudSignIn { id, .. } => id,
            Self::ProvidersCheckKey { id, .. } => id,
            Self::BrokerStatus { id, .. } => id,
            Self::AgentsAddTools { id, .. } => id,
            Self::AgentsSetup { id, .. } => id,
            Self::SkillsSearch { id, .. } => id,
            Self::SkillsGet { id, .. } => id,
            Self::SkillsPreview { id, .. } => id,
            Self::SkillsAdd { id, .. } => id,
            Self::SkillsRemove { id, .. } => id,
            Self::SkillsToggle { id, .. } => id,
            Self::ServersAdd { id, .. } => id,
            Self::ServersRemove { id, .. } => id,
            Self::ServersToggle { id, .. } => id,
            Self::PluginsToggle { id, .. } => id,
            Self::InitGet { id, .. } => id,
            Self::InitKeys { id, .. } => id,
            Self::PreferencesGet { id, .. } => id,
            Self::PreferencesSet { id, .. } => id,
            Self::ReleaseGet { id, .. } => id,
            Self::ReleaseCheck { id, .. } => id,
            Self::HostRestart { id, .. } => id,
            Self::ProjectsAdd { id, .. } => id,
            Self::ProjectSeedPlan { id, .. } => id,
            Self::ProjectsSeed { id, .. } => id,
            Self::ProjectsList { id, .. } => id,
            Self::ConversationsList { id, .. } => id,
            Self::ConversationsRead { id, .. } => id,
            Self::ProjectsDefaults { id, .. } => id,
            Self::ProjectsResolve { id, .. } => id,
            Self::ProjectsBranch { id, .. } => id,
            Self::ProjectsRemove { id, .. } => id,
            Self::ProjectsIcon { id, .. } => id,
            Self::ProjectsIcons { id, .. } => id,
            Self::ProjectPlan { id, .. } => id,
            Self::ProjectImport { id, .. } => id,
            Self::ProjectExport { id, .. } => id,
            Self::ImageGet { id, .. } => id,
            Self::ImageBuild { id, .. } => id,
            Self::SlatesGet { id, .. } => id,
            Self::SlatesWrite { id, .. } => id,
            Self::SlatesState { id, .. } => id,
            Self::SlatesRead { id, .. } => id,
            Self::SlatesCatalog { id, .. } => id,
            Self::SlatesEvent { id, .. } => id,
            Self::SlatesApprove { id, .. } => id,
            Self::SlatesCancel { id, .. } => id,
            Self::SlatesRevoke { id, .. } => id,
            Self::SlatesShown { id, .. } => id,
            Self::SlatesSubscribe { id, .. } => id,
            Self::SlatesUnsubscribe { id, .. } => id,
            Self::SlatesResolve { id, .. } => id,
            Self::SlatesImage { id, .. } => id,
        }
    }
}
