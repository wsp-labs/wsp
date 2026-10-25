// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/events-union.ts: every event the host pushes on its bus, and the reply to events.subscribe.

use crate::types::js_number::JsInt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::agents_report::AgentsChangedEvent;
use crate::types::check::{leaf, num_above, num_at_least, parse, Zod};
use crate::types::init_job::{InitJobEvent, InitNeedsYouEvent};
use crate::types::plan_alerts::UsageAlertEvent;
use crate::types::release::ReleaseChangedEvent;
use crate::types::slate_wire::{SessionSlateEvent, SlateRunEvent, SlateValuesEvent};
use crate::types::usage::UsageAccountEvent;
use crate::types::{Check, SchemaError};
use crate::views::golden_image::GoldenStageEvent;
use crate::views::place::{
    AsideTextEvent, ForwardCloseEvent, ForwardOpenEvent, HostNoticeEvent, PlaceAbsentEvent, PlaceChangedEvent, PlaceJoinedEvent,
    PlacePendingEvent, PlacePresentEvent, PlaceRemovedEvent, PlaceSetupEvent, PlaceStageEvent, PlaceSyncEvent, ProjectAddEvent,
    ProjectAddedEvent, ProjectRemovedEvent, RecipesChangedEvent,
};
use crate::views::preferences::{PreferencesChangedEvent, ThreadHeadEvent, ThreadMarkedEvent, ThreadRewoundEvent};
use crate::views::project_bundle::{ProjectExportEvent, ProjectImportEvent};
use crate::views::session_events::{
    SessionBehindEvent, SessionCappedEvent, SessionChangesEvent, SessionCheckpointEvent, SessionCompactedEvent, SessionContextEvent,
    SessionDeltaEvent, SessionDoneEvent, SessionEarlierEvent, SessionEndEvent, SessionHeldEvent, SessionMovedEvent, SessionNoticeEvent,
    SessionNotifyEvent, SessionPermissionClosedEvent, SessionPermissionEvent, SessionPlanEvent, SessionQueuedEvent, SessionRetryEvent,
    SessionRowEvent, SessionRunEvent, SessionStartEvent, SessionStartingEvent, SessionSteerEvent, SessionSubagentEvent,
    SessionUncappedEvent, ThreadWaitedEvent,
};
use crate::views::workspace_events::{
    InboxFileEvent, PortCloseEvent, PortOpenEvent, WorkspaceCostEvent, WorkspaceCreatedEvent, WorkspaceCreatingEvent,
    WorkspaceDeletedEvent, WorkspaceGoneEvent, WorkspaceLookEvent, WorkspaceNappedEvent, WorkspaceReleasedEvent, WorkspaceRenamedEvent,
    WorkspaceReviewEvent, WorkspaceStatusEvent, WorkspaceUpgradedEvent, WorkspaceViewedEvent, WorkspaceWokenEvent,
};

/// zod's `.extend(sequenced)`: an event with the place it holds in its runtime's stream. Absent on an event from an
/// older runtime and on a sessions.history reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sequenced<E> {
    #[serde(flatten)]
    pub event: E,
    /// A positive integer, counted from 1 per runtime process (zod: int, positive).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seq: Option<JsInt>,
}

/// Every event the bus carries, one variant per member of the TS union, in its order; the `type` key names the
/// variant and is written first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventUnion {
    #[serde(rename = "workspace.creating")]
    WorkspaceCreating(Sequenced<WorkspaceCreatingEvent>),
    #[serde(rename = "workspace.created")]
    WorkspaceCreated(Sequenced<WorkspaceCreatedEvent>),
    #[serde(rename = "workspace.napped")]
    WorkspaceNapped(Sequenced<WorkspaceNappedEvent>),
    #[serde(rename = "workspace.woken")]
    WorkspaceWoken(Sequenced<WorkspaceWokenEvent>),
    #[serde(rename = "workspace.upgraded")]
    WorkspaceUpgraded(Sequenced<WorkspaceUpgradedEvent>),
    #[serde(rename = "workspace.renamed")]
    WorkspaceRenamed(Sequenced<WorkspaceRenamedEvent>),
    #[serde(rename = "workspace.look")]
    WorkspaceLook(Sequenced<WorkspaceLookEvent>),
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted(Sequenced<WorkspaceDeletedEvent>),
    #[serde(rename = "workspace.gone")]
    WorkspaceGone(Sequenced<WorkspaceGoneEvent>),
    #[serde(rename = "workspace.released")]
    WorkspaceReleased(Sequenced<WorkspaceReleasedEvent>),
    #[serde(rename = "workspace.status")]
    WorkspaceStatus(Sequenced<WorkspaceStatusEvent>),
    #[serde(rename = "workspace.review")]
    WorkspaceReview(Sequenced<WorkspaceReviewEvent>),
    #[serde(rename = "workspace.viewed")]
    WorkspaceViewed(Sequenced<WorkspaceViewedEvent>),
    #[serde(rename = "workspace.cost")]
    WorkspaceCost(Sequenced<WorkspaceCostEvent>),
    #[serde(rename = "session.start")]
    SessionStart(Sequenced<SessionStartEvent>),
    #[serde(rename = "session.delta")]
    SessionDelta(Sequenced<SessionDeltaEvent>),
    #[serde(rename = "session.done")]
    SessionDone(Sequenced<SessionDoneEvent>),
    #[serde(rename = "session.end")]
    SessionEnd(Sequenced<SessionEndEvent>),
    #[serde(rename = "session.steer")]
    SessionSteer(Sequenced<SessionSteerEvent>),
    #[serde(rename = "session.notify")]
    SessionNotify(Sequenced<SessionNotifyEvent>),
    #[serde(rename = "session.permission")]
    SessionPermission(Sequenced<SessionPermissionEvent>),
    #[serde(rename = "session.permission.closed")]
    SessionPermissionClosed(Sequenced<SessionPermissionClosedEvent>),
    #[serde(rename = "session.checkpoint")]
    SessionCheckpoint(Sequenced<SessionCheckpointEvent>),
    #[serde(rename = "session.changes")]
    SessionChanges(Sequenced<SessionChangesEvent>),
    #[serde(rename = "session.plan")]
    SessionPlan(Sequenced<SessionPlanEvent>),
    #[serde(rename = "session.compacted")]
    SessionCompacted(Sequenced<SessionCompactedEvent>),
    #[serde(rename = "session.retry")]
    SessionRetry(Sequenced<SessionRetryEvent>),
    #[serde(rename = "session.context")]
    SessionContext(Sequenced<SessionContextEvent>),
    #[serde(rename = "session.run")]
    SessionRun(Sequenced<SessionRunEvent>),
    #[serde(rename = "session.moved")]
    SessionMoved(Sequenced<SessionMovedEvent>),
    #[serde(rename = "session.behind")]
    SessionBehind(Sequenced<SessionBehindEvent>),
    #[serde(rename = "session.notice")]
    SessionNotice(Sequenced<SessionNoticeEvent>),
    #[serde(rename = "session.earlier")]
    SessionEarlier(Sequenced<SessionEarlierEvent>),
    #[serde(rename = "session.capped")]
    SessionCapped(Sequenced<SessionCappedEvent>),
    #[serde(rename = "session.uncapped")]
    SessionUncapped(Sequenced<SessionUncappedEvent>),
    #[serde(rename = "session.subagent")]
    SessionSubagent(Sequenced<SessionSubagentEvent>),
    #[serde(rename = "session.slate")]
    SessionSlate(Sequenced<SessionSlateEvent>),
    #[serde(rename = "thread.waited")]
    ThreadWaited(Sequenced<ThreadWaitedEvent>),
    #[serde(rename = "slate.values")]
    SlateValues(Sequenced<SlateValuesEvent>),
    #[serde(rename = "slate.run")]
    SlateRun(Sequenced<SlateRunEvent>),
    #[serde(rename = "usage.account")]
    UsageAccount(Sequenced<UsageAccountEvent>),
    #[serde(rename = "session.queued")]
    SessionQueued(Sequenced<SessionQueuedEvent>),
    #[serde(rename = "session.held")]
    SessionHeld(Sequenced<SessionHeldEvent>),
    #[serde(rename = "session.row")]
    SessionRow(Sequenced<SessionRowEvent>),
    #[serde(rename = "session.starting")]
    SessionStarting(Sequenced<SessionStartingEvent>),
    #[serde(rename = "thread.marked")]
    ThreadMarked(Sequenced<ThreadMarkedEvent>),
    #[serde(rename = "thread.head")]
    ThreadHead(Sequenced<ThreadHeadEvent>),
    #[serde(rename = "thread.rewound")]
    ThreadRewound(Sequenced<ThreadRewoundEvent>),
    #[serde(rename = "port.open")]
    PortOpen(Sequenced<PortOpenEvent>),
    #[serde(rename = "port.close")]
    PortClose(Sequenced<PortCloseEvent>),
    #[serde(rename = "inbox.file")]
    InboxFile(Sequenced<InboxFileEvent>),
    #[serde(rename = "golden.stage")]
    GoldenStage(Sequenced<GoldenStageEvent>),
    #[serde(rename = "forward.open")]
    ForwardOpen(Sequenced<ForwardOpenEvent>),
    #[serde(rename = "forward.close")]
    ForwardClose(Sequenced<ForwardCloseEvent>),
    #[serde(rename = "project.added")]
    ProjectAdded(Sequenced<ProjectAddedEvent>),
    #[serde(rename = "project.add")]
    ProjectAdd(Sequenced<ProjectAddEvent>),
    #[serde(rename = "project.removed")]
    ProjectRemoved(Sequenced<ProjectRemovedEvent>),
    #[serde(rename = "project.import")]
    ProjectImport(Sequenced<ProjectImportEvent>),
    #[serde(rename = "project.export")]
    ProjectExport(Sequenced<ProjectExportEvent>),
    #[serde(rename = "preferences.changed")]
    PreferencesChanged(Sequenced<PreferencesChangedEvent>),
    #[serde(rename = "release.changed")]
    ReleaseChanged(Sequenced<ReleaseChangedEvent>),
    #[serde(rename = "init.job")]
    InitJob(Sequenced<InitJobEvent>),
    #[serde(rename = "job.needs-you")]
    JobNeedsYou(Sequenced<InitNeedsYouEvent>),
    #[serde(rename = "place.stage")]
    PlaceStage(Sequenced<PlaceStageEvent>),
    #[serde(rename = "place.setup")]
    PlaceSetup(Sequenced<PlaceSetupEvent>),
    #[serde(rename = "place.sync")]
    PlaceSync(Sequenced<PlaceSyncEvent>),
    #[serde(rename = "recipes.changed")]
    RecipesChanged(Sequenced<RecipesChangedEvent>),
    #[serde(rename = "place.pending")]
    PlacePending(Sequenced<PlacePendingEvent>),
    #[serde(rename = "place.joined")]
    PlaceJoined(Sequenced<PlaceJoinedEvent>),
    #[serde(rename = "place.present")]
    PlacePresent(Sequenced<PlacePresentEvent>),
    #[serde(rename = "place.absent")]
    PlaceAbsent(Sequenced<PlaceAbsentEvent>),
    #[serde(rename = "place.removed")]
    PlaceRemoved(Sequenced<PlaceRemovedEvent>),
    #[serde(rename = "place.changed")]
    PlaceChanged(Sequenced<PlaceChangedEvent>),
    #[serde(rename = "agents.changed")]
    AgentsChanged(Sequenced<AgentsChangedEvent>),
    #[serde(rename = "usage.alert")]
    UsageAlert(Sequenced<UsageAlertEvent>),
    #[serde(rename = "host.notice")]
    HostNotice(Sequenced<HostNoticeEvent>),
    #[serde(rename = "aside.text")]
    AsideText(Sequenced<AsideTextEvent>),
}

/// What events.subscribe answers before it pushes anything: the newest sequence issued (0 before the first event),
/// the runtime process that issued it, and `gap` when the `after` sent is no cursor into this stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventsSubscribeReply {
    /// zod: int, nonnegative; a negative one is too_small at ["seq"], so it reads as JsInt and Check refuses it.
    pub seq: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gap: Option<wsp_frames::True>,
}

/// Refinements: `seq` a positive integer where present (0 is too_small, "Number must be greater than 0"); every variant's own event checks as its views type says.
impl<E: Check> Check for Sequenced<E> {
    fn check(&self) -> Result<(), SchemaError> {
        self.event.check()?;
        self.seq.map_or(Ok(()), |seq| num_above(seq.as_f64(), 0.0).map_err(|e| e.under("seq")))
    }

    /// The event's own fields come first, then `seq`, as the schema's shape order has them.
    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut body = obj.clone();
        body.shift_remove("seq");
        let mut out = crate::types::check::issues_of::<E>(&Value::Object(body));
        crate::types::keys::opt_key_with::<JsInt>(obj, "seq", &mut out, |seq| seq.map_or(Ok(()), |n| num_above(n.as_f64(), 0.0)));
        Some(out)
    }
}

/// No refinement of its own; checks the event inside.
impl Check for EventUnion {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            EventUnion::WorkspaceCreating(v) => v.check(),
            EventUnion::WorkspaceCreated(v) => v.check(),
            EventUnion::WorkspaceNapped(v) => v.check(),
            EventUnion::WorkspaceWoken(v) => v.check(),
            EventUnion::WorkspaceUpgraded(v) => v.check(),
            EventUnion::WorkspaceRenamed(v) => v.check(),
            EventUnion::WorkspaceLook(v) => v.check(),
            EventUnion::WorkspaceDeleted(v) => v.check(),
            EventUnion::WorkspaceGone(v) => v.check(),
            EventUnion::WorkspaceReleased(v) => v.check(),
            EventUnion::WorkspaceStatus(v) => v.check(),
            EventUnion::WorkspaceReview(v) => v.check(),
            EventUnion::WorkspaceViewed(v) => v.check(),
            EventUnion::WorkspaceCost(v) => v.check(),
            EventUnion::SessionStart(v) => v.check(),
            EventUnion::SessionDelta(v) => v.check(),
            EventUnion::SessionDone(v) => v.check(),
            EventUnion::SessionEnd(v) => v.check(),
            EventUnion::SessionSteer(v) => v.check(),
            EventUnion::SessionNotify(v) => v.check(),
            EventUnion::SessionPermission(v) => v.check(),
            EventUnion::SessionPermissionClosed(v) => v.check(),
            EventUnion::SessionCheckpoint(v) => v.check(),
            EventUnion::SessionChanges(v) => v.check(),
            EventUnion::SessionPlan(v) => v.check(),
            EventUnion::SessionCompacted(v) => v.check(),
            EventUnion::SessionRetry(v) => v.check(),
            EventUnion::SessionContext(v) => v.check(),
            EventUnion::SessionRun(v) => v.check(),
            EventUnion::SessionMoved(v) => v.check(),
            EventUnion::SessionBehind(v) => v.check(),
            EventUnion::SessionNotice(v) => v.check(),
            EventUnion::SessionEarlier(v) => v.check(),
            EventUnion::SessionCapped(v) => v.check(),
            EventUnion::SessionUncapped(v) => v.check(),
            EventUnion::SessionSubagent(v) => v.check(),
            EventUnion::SessionSlate(v) => v.check(),
            EventUnion::ThreadWaited(v) => v.check(),
            EventUnion::SlateValues(v) => v.check(),
            EventUnion::SlateRun(v) => v.check(),
            EventUnion::UsageAccount(v) => v.check(),
            EventUnion::SessionQueued(v) => v.check(),
            EventUnion::SessionHeld(v) => v.check(),
            EventUnion::SessionRow(v) => v.check(),
            EventUnion::SessionStarting(v) => v.check(),
            EventUnion::ThreadMarked(v) => v.check(),
            EventUnion::ThreadHead(v) => v.check(),
            EventUnion::ThreadRewound(v) => v.check(),
            EventUnion::PortOpen(v) => v.check(),
            EventUnion::PortClose(v) => v.check(),
            EventUnion::InboxFile(v) => v.check(),
            EventUnion::GoldenStage(v) => v.check(),
            EventUnion::ForwardOpen(v) => v.check(),
            EventUnion::ForwardClose(v) => v.check(),
            EventUnion::ProjectAdded(v) => v.check(),
            EventUnion::ProjectAdd(v) => v.check(),
            EventUnion::ProjectRemoved(v) => v.check(),
            EventUnion::ProjectImport(v) => v.check(),
            EventUnion::ProjectExport(v) => v.check(),
            EventUnion::PreferencesChanged(v) => v.check(),
            EventUnion::ReleaseChanged(v) => v.check(),
            EventUnion::InitJob(v) => v.check(),
            EventUnion::JobNeedsYou(v) => v.check(),
            EventUnion::PlaceStage(v) => v.check(),
            EventUnion::PlaceSetup(v) => v.check(),
            EventUnion::PlaceSync(v) => v.check(),
            EventUnion::RecipesChanged(v) => v.check(),
            EventUnion::PlacePending(v) => v.check(),
            EventUnion::PlaceJoined(v) => v.check(),
            EventUnion::PlacePresent(v) => v.check(),
            EventUnion::PlaceAbsent(v) => v.check(),
            EventUnion::PlaceRemoved(v) => v.check(),
            EventUnion::PlaceChanged(v) => v.check(),
            EventUnion::AgentsChanged(v) => v.check(),
            EventUnion::UsageAlert(v) => v.check(),
            EventUnion::HostNotice(v) => v.check(),
            EventUnion::AsideText(v) => v.check(),
        }
    }
    fn describe(path: &[Value]) -> Option<Zod> {
        crate::types::check::tag_describe(path, "type", EVENT_TYPES)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else {
            return Some(vec![crate::types::check::missing::<Self>("type")]);
        };
        match tag.as_str() {
            Some("workspace.creating") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceCreatingEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.created") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceCreatedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.napped") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceNappedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.woken") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceWokenEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.upgraded") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceUpgradedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.renamed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceRenamedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.look") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceLookEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.deleted") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceDeletedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.gone") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceGoneEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.released") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceReleasedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.status") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceStatusEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.review") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceReviewEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.viewed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceViewedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("workspace.cost") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<WorkspaceCostEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.start") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionStartEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.delta") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionDeltaEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.done") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionDoneEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.end") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionEndEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.steer") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionSteerEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.notify") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionNotifyEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.permission") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionPermissionEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.permission.closed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionPermissionClosedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.checkpoint") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionCheckpointEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.changes") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionChangesEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.plan") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionPlanEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.compacted") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionCompactedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.retry") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionRetryEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.context") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionContextEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.run") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionRunEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.moved") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionMovedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.behind") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionBehindEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.notice") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionNoticeEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.earlier") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionEarlierEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.capped") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionCappedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.uncapped") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionUncappedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.subagent") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionSubagentEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.slate") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionSlateEvent>>(&serde_json::Value::Object(body)))
            }
            Some("thread.waited") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ThreadWaitedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("slate.values") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SlateValuesEvent>>(&serde_json::Value::Object(body)))
            }
            Some("slate.run") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SlateRunEvent>>(&serde_json::Value::Object(body)))
            }
            Some("usage.account") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<UsageAccountEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.queued") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionQueuedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.held") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionHeldEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.row") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionRowEvent>>(&serde_json::Value::Object(body)))
            }
            Some("session.starting") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<SessionStartingEvent>>(&serde_json::Value::Object(body)))
            }
            Some("thread.marked") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ThreadMarkedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("thread.head") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ThreadHeadEvent>>(&serde_json::Value::Object(body)))
            }
            Some("thread.rewound") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ThreadRewoundEvent>>(&serde_json::Value::Object(body)))
            }
            Some("port.open") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PortOpenEvent>>(&serde_json::Value::Object(body)))
            }
            Some("port.close") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PortCloseEvent>>(&serde_json::Value::Object(body)))
            }
            Some("inbox.file") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<InboxFileEvent>>(&serde_json::Value::Object(body)))
            }
            Some("golden.stage") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<GoldenStageEvent>>(&serde_json::Value::Object(body)))
            }
            Some("forward.open") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ForwardOpenEvent>>(&serde_json::Value::Object(body)))
            }
            Some("forward.close") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ForwardCloseEvent>>(&serde_json::Value::Object(body)))
            }
            Some("project.added") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ProjectAddedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("project.add") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ProjectAddEvent>>(&serde_json::Value::Object(body)))
            }
            Some("project.removed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ProjectRemovedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("project.import") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ProjectImportEvent>>(&serde_json::Value::Object(body)))
            }
            Some("project.export") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ProjectExportEvent>>(&serde_json::Value::Object(body)))
            }
            Some("preferences.changed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PreferencesChangedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("release.changed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<ReleaseChangedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("init.job") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<InitJobEvent>>(&serde_json::Value::Object(body)))
            }
            Some("job.needs-you") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<InitNeedsYouEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.stage") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceStageEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.setup") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceSetupEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.sync") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceSyncEvent>>(&serde_json::Value::Object(body)))
            }
            Some("recipes.changed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<RecipesChangedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.pending") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlacePendingEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.joined") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceJoinedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.present") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlacePresentEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.absent") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceAbsentEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.removed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceRemovedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("place.changed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<PlaceChangedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("agents.changed") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<AgentsChangedEvent>>(&serde_json::Value::Object(body)))
            }
            Some("usage.alert") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<UsageAlertEvent>>(&serde_json::Value::Object(body)))
            }
            Some("host.notice") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<HostNoticeEvent>>(&serde_json::Value::Object(body)))
            }
            Some("aside.text") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<Sequenced<AsideTextEvent>>(&serde_json::Value::Object(body)))
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }
}

/// Refinement: `seq` non-negative (too_small, "Number must be greater than or equal to 0").
impl Check for EventsSubscribeReply {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.seq.as_f64(), 0.0).map_err(|e| e.under("seq"))
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "seq", &mut out, Self::rule_seq);
        crate::types::keys::opt_key::<String>(obj, "stream", &mut out);
        crate::types::keys::opt_key::<wsp_frames::True>(obj, "gap", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "seq" => <i64 as Check>::describe(rest),
                "stream" => <Option<String> as Check>::describe(rest),
                "gap" => leaf(rest, Zod::Literal(|| Value::Bool(true))),
                _ => None,
            },
        }
    }
}
impl EventsSubscribeReply {
    fn rule_seq(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
}

/// One event as the bus carries it, read as zod reads it: an unknown `type` is one invalid_union_discriminator issue on
/// ["type"] naming every type the union holds, in its order; anything else is the first failing field's issue.
pub fn parse_event(value: &Value) -> Result<EventUnion, SchemaError> {
    parse::<EventUnion>(value)
}

/// Every event type the union holds, in its order.
pub const EVENT_TYPES: &[&str] = &[
    "workspace.creating",
    "workspace.created",
    "workspace.napped",
    "workspace.woken",
    "workspace.upgraded",
    "workspace.renamed",
    "workspace.look",
    "workspace.deleted",
    "workspace.gone",
    "workspace.released",
    "workspace.status",
    "workspace.review",
    "workspace.viewed",
    "workspace.cost",
    "session.start",
    "session.delta",
    "session.done",
    "session.end",
    "session.steer",
    "session.notify",
    "session.permission",
    "session.permission.closed",
    "session.checkpoint",
    "session.changes",
    "session.plan",
    "session.compacted",
    "session.retry",
    "session.context",
    "session.run",
    "session.moved",
    "session.behind",
    "session.notice",
    "session.earlier",
    "session.capped",
    "session.uncapped",
    "session.subagent",
    "session.slate",
    "thread.waited",
    "slate.values",
    "slate.run",
    "usage.account",
    "session.queued",
    "session.held",
    "session.row",
    "session.starting",
    "thread.marked",
    "thread.head",
    "thread.rewound",
    "port.open",
    "port.close",
    "inbox.file",
    "golden.stage",
    "forward.open",
    "forward.close",
    "project.added",
    "project.add",
    "project.removed",
    "project.import",
    "project.export",
    "preferences.changed",
    "release.changed",
    "init.job",
    "job.needs-you",
    "place.stage",
    "place.setup",
    "place.sync",
    "recipes.changed",
    "place.pending",
    "place.joined",
    "place.present",
    "place.absent",
    "place.removed",
    "place.changed",
    "agents.changed",
    "usage.alert",
    "host.notice",
    "aside.text",
];
