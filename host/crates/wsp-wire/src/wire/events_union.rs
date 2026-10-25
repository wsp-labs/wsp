// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/events-union.ts: every event the host pushes on its bus, and the reply to events.subscribe.

use serde::{Deserialize, Serialize};

use crate::types::agents_report::AgentsChangedEvent;
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
    pub seq: Option<i64>,
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
    /// zod: int, nonnegative; a negative one is too_small at ["seq"], so it reads as i64 and Check refuses it.
    pub seq: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gap: Option<wsp_frames::True>,
}

/// Refinements: `seq` a positive integer where present (0 is too_small, "Number must be greater than 0"); every variant's own event checks as its views type says.
impl<E: Check> Check for Sequenced<E> {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks the event inside.
impl Check for EventUnion {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinement: `seq` non-negative (too_small, "Number must be greater than or equal to 0").
impl Check for EventsSubscribeReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// One event as the bus carries it, read as zod reads it: an unknown `type` is one invalid_union_discriminator issue on
/// ["type"] naming every type the union holds, in its order; anything else is the first failing field's issue.
pub fn parse_event(value: &serde_json::Value) -> Result<EventUnion, SchemaError> {
    let _ = value;
    todo!()
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
