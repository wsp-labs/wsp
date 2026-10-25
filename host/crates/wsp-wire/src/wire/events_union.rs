// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/events-union.ts: every event the host pushes on its bus, and the reply to events.subscribe.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "read_gap")]
    pub gap: Option<wsp_frames::True>,
}

fn read_gap<'de, D: Deserializer<'de>>(d: D) -> Result<Option<wsp_frames::True>, D::Error> {
    if Value::deserialize(d)? == Value::Bool(true) {
        return Ok(Some(wsp_frames::True));
    }
    Err(serde::de::Error::custom("Invalid literal value, expected true"))
}

/// Refinements: `seq` a positive integer where present (0 is too_small, "Number must be greater than 0"); every variant's own event checks as its views type says.
impl<E: Check> Check for Sequenced<E> {
    fn check(&self) -> Result<(), SchemaError> {
        self.event.check()?;
        self.seq.map_or(Ok(()), |seq| num_above(seq as f64, 0.0).map_err(|e| e.under("seq")))
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
}

/// Refinement: `seq` non-negative (too_small, "Number must be greater than or equal to 0").
impl Check for EventsSubscribeReply {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.seq as f64, 0.0).map_err(|e| e.under("seq"))
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

/// One event as the bus carries it, read as zod reads it: an unknown `type` is one invalid_union_discriminator issue on
/// ["type"] naming every type the union holds, in its order; anything else is the first failing field's issue.
pub fn parse_event(value: &Value) -> Result<EventUnion, SchemaError> {
    let Value::Object(obj) = value else { return Err(not_an_object(value)) };
    let tag = obj.get("type").and_then(Value::as_str).unwrap_or_default();
    let mut body = obj.clone();
    body.shift_remove("type");
    let seq = body.shift_remove("seq");
    match tag {
        "workspace.creating" => sequenced(body, seq).map(EventUnion::WorkspaceCreating),
        "workspace.created" => sequenced(body, seq).map(EventUnion::WorkspaceCreated),
        "workspace.napped" => sequenced(body, seq).map(EventUnion::WorkspaceNapped),
        "workspace.woken" => sequenced(body, seq).map(EventUnion::WorkspaceWoken),
        "workspace.upgraded" => sequenced(body, seq).map(EventUnion::WorkspaceUpgraded),
        "workspace.renamed" => sequenced(body, seq).map(EventUnion::WorkspaceRenamed),
        "workspace.look" => sequenced(body, seq).map(EventUnion::WorkspaceLook),
        "workspace.deleted" => sequenced(body, seq).map(EventUnion::WorkspaceDeleted),
        "workspace.gone" => sequenced(body, seq).map(EventUnion::WorkspaceGone),
        "workspace.released" => sequenced(body, seq).map(EventUnion::WorkspaceReleased),
        "workspace.status" => sequenced(body, seq).map(EventUnion::WorkspaceStatus),
        "workspace.review" => sequenced(body, seq).map(EventUnion::WorkspaceReview),
        "workspace.viewed" => sequenced(body, seq).map(EventUnion::WorkspaceViewed),
        "workspace.cost" => sequenced(body, seq).map(EventUnion::WorkspaceCost),
        "session.start" => sequenced(body, seq).map(EventUnion::SessionStart),
        "session.delta" => sequenced(body, seq).map(EventUnion::SessionDelta),
        "session.done" => sequenced(body, seq).map(EventUnion::SessionDone),
        "session.end" => sequenced(body, seq).map(EventUnion::SessionEnd),
        "session.steer" => sequenced(body, seq).map(EventUnion::SessionSteer),
        "session.notify" => sequenced(body, seq).map(EventUnion::SessionNotify),
        "session.permission" => sequenced(body, seq).map(EventUnion::SessionPermission),
        "session.permission.closed" => sequenced(body, seq).map(EventUnion::SessionPermissionClosed),
        "session.checkpoint" => sequenced(body, seq).map(EventUnion::SessionCheckpoint),
        "session.changes" => sequenced(body, seq).map(EventUnion::SessionChanges),
        "session.plan" => sequenced(body, seq).map(EventUnion::SessionPlan),
        "session.compacted" => sequenced(body, seq).map(EventUnion::SessionCompacted),
        "session.retry" => sequenced(body, seq).map(EventUnion::SessionRetry),
        "session.context" => sequenced(body, seq).map(EventUnion::SessionContext),
        "session.run" => sequenced(body, seq).map(EventUnion::SessionRun),
        "session.moved" => sequenced(body, seq).map(EventUnion::SessionMoved),
        "session.behind" => sequenced(body, seq).map(EventUnion::SessionBehind),
        "session.notice" => sequenced(body, seq).map(EventUnion::SessionNotice),
        "session.earlier" => sequenced(body, seq).map(EventUnion::SessionEarlier),
        "session.capped" => sequenced(body, seq).map(EventUnion::SessionCapped),
        "session.uncapped" => sequenced(body, seq).map(EventUnion::SessionUncapped),
        "session.subagent" => sequenced(body, seq).map(EventUnion::SessionSubagent),
        "session.slate" => sequenced(body, seq).map(EventUnion::SessionSlate),
        "thread.waited" => sequenced(body, seq).map(EventUnion::ThreadWaited),
        "slate.values" => sequenced(body, seq).map(EventUnion::SlateValues),
        "slate.run" => sequenced(body, seq).map(EventUnion::SlateRun),
        "usage.account" => sequenced(body, seq).map(EventUnion::UsageAccount),
        "session.queued" => sequenced(body, seq).map(EventUnion::SessionQueued),
        "session.held" => sequenced(body, seq).map(EventUnion::SessionHeld),
        "session.row" => sequenced(body, seq).map(EventUnion::SessionRow),
        "session.starting" => sequenced(body, seq).map(EventUnion::SessionStarting),
        "thread.marked" => sequenced(body, seq).map(EventUnion::ThreadMarked),
        "thread.head" => sequenced(body, seq).map(EventUnion::ThreadHead),
        "thread.rewound" => sequenced(body, seq).map(EventUnion::ThreadRewound),
        "port.open" => sequenced(body, seq).map(EventUnion::PortOpen),
        "port.close" => sequenced(body, seq).map(EventUnion::PortClose),
        "inbox.file" => sequenced(body, seq).map(EventUnion::InboxFile),
        "golden.stage" => sequenced(body, seq).map(EventUnion::GoldenStage),
        "forward.open" => sequenced(body, seq).map(EventUnion::ForwardOpen),
        "forward.close" => sequenced(body, seq).map(EventUnion::ForwardClose),
        "project.added" => sequenced(body, seq).map(EventUnion::ProjectAdded),
        "project.add" => sequenced(body, seq).map(EventUnion::ProjectAdd),
        "project.removed" => sequenced(body, seq).map(EventUnion::ProjectRemoved),
        "project.import" => sequenced(body, seq).map(EventUnion::ProjectImport),
        "project.export" => sequenced(body, seq).map(EventUnion::ProjectExport),
        "preferences.changed" => sequenced(body, seq).map(EventUnion::PreferencesChanged),
        "release.changed" => sequenced(body, seq).map(EventUnion::ReleaseChanged),
        "init.job" => sequenced(body, seq).map(EventUnion::InitJob),
        "job.needs-you" => sequenced(body, seq).map(EventUnion::JobNeedsYou),
        "place.stage" => sequenced(body, seq).map(EventUnion::PlaceStage),
        "place.setup" => sequenced(body, seq).map(EventUnion::PlaceSetup),
        "place.sync" => sequenced(body, seq).map(EventUnion::PlaceSync),
        "recipes.changed" => sequenced(body, seq).map(EventUnion::RecipesChanged),
        "place.pending" => sequenced(body, seq).map(EventUnion::PlacePending),
        "place.joined" => sequenced(body, seq).map(EventUnion::PlaceJoined),
        "place.present" => sequenced(body, seq).map(EventUnion::PlacePresent),
        "place.absent" => sequenced(body, seq).map(EventUnion::PlaceAbsent),
        "place.removed" => sequenced(body, seq).map(EventUnion::PlaceRemoved),
        "place.changed" => sequenced(body, seq).map(EventUnion::PlaceChanged),
        "agents.changed" => sequenced(body, seq).map(EventUnion::AgentsChanged),
        "usage.alert" => sequenced(body, seq).map(EventUnion::UsageAlert),
        "host.notice" => sequenced(body, seq).map(EventUnion::HostNotice),
        "aside.text" => sequenced(body, seq).map(EventUnion::AsideText),
        _ => Err(union_discriminator(EVENT_TYPES)),
    }
}

/// The event's own fields are read first, so its issues come before `seq`'s, as zod's shape order has them.
fn sequenced<E: DeserializeOwned + Check>(body: Map<String, Value>, seq: Option<Value>) -> Result<Sequenced<E>, SchemaError> {
    let event = parse(&Value::Object(body))?;
    Ok(Sequenced { event, seq: read_seq(seq)? })
}

fn read_seq(seq: Option<Value>) -> Result<Option<i64>, SchemaError> {
    let Some(value) = seq else { return Ok(None) };
    match value.as_f64() {
        None => Err(type_issue(vec![Value::from("seq")], "number", received_word(&value))),
        Some(n) if n.fract() != 0.0 => Err(type_issue(vec![Value::from("seq")], "integer", "float")),
        Some(n) => num_above(n, 0.0).map(|()| Some(n as i64)).map_err(|e| e.under("seq")),
    }
}

pub(crate) fn union_discriminator(options: &[&str]) -> SchemaError {
    let words = options.iter().map(|o| format!("'{o}'")).collect::<Vec<_>>().join(" | ");
    let mut detail = IndexMap::new();
    detail.insert("options".to_string(), Value::Array(options.iter().map(|o| Value::from(*o)).collect()));
    SchemaError {
        code: "invalid_union_discriminator".into(),
        path: vec![Value::from("type")],
        message: format!("Invalid discriminator value. Expected {words}"),
        detail,
    }
}

pub(crate) fn not_an_object(value: &Value) -> SchemaError {
    type_issue(Vec::new(), "object", received_word(value))
}

pub(crate) fn type_issue(path: Vec<Value>, expected: &str, received: &str) -> SchemaError {
    let mut detail = IndexMap::new();
    detail.insert("expected".to_string(), Value::from(expected));
    detail.insert("received".to_string(), Value::from(received));
    SchemaError { code: "invalid_type".into(), path, message: format!("Expected {expected}, received {received}"), detail }
}

pub(crate) fn received_word(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
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
