// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/types.ts` at 65f6b89c7: the place record, the roads a host wires for its
//! places, the one road into the runtime a place needs, the door's own interface, its lines and its error marks.
//! Spec: `spec/types.md`.
//!
//! One thread (spec 03, rule 3.1): every trait here is `?Send` and every road is held as `Rc<dyn ..>`. A TS
//! optional method becomes an `Option<Rc<dyn ..>>` field on `PlaceWiring`, or a `PlaceRecording` method answering
//! `None` for "absent" (each such method says so).

use std::rc::Rc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use wsp_engine::machine::{ExecResult, Machine, MachineBackend};
use wsp_engine::place_machine::PlaceFolderMachine;
use wsp_engine::provision::{EngineStep, ProvisionOn, ProvisionPlan, ProvisionStage, SetupRun};
use wsp_store::keys::{PlaceKeyPair, Seal};
use wsp_wire::types::agents_report::{AgentSignInState, AgentsSignInEvent, SignInLine};
use wsp_wire::types::recipe_file::{RecipeFile, RecipeFileFoldersValue};
use wsp_wire::views::place::{
    PendingComputer, PlaceAddStep, PlaceApplied, PlaceBack, PlaceDialled, PlaceProvisionRow, PlaceRoad, PlaceSetAlso,
    PlaceSetup, PlaceSetupStep, PlaceStageEvent, PlaceSync, PlaceAddJob,
};
use wsp_wire::views::place_more::{
    MacKind, PlaceDial, PlaceHolds, PlaceRemoved, PlaceRemovedTook, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk,
    PlaceUpdateReply, PlaceView,
};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::{BackendFacts, SysSample};
use wsp_wire::wire::place_link::{
    PlaceAuthRefusal, PlaceAuthReply, PlaceAuthRequest, PlaceJoinReply, PlaceJoinRequest, PlaceProveRequest, PlaceReport,
};
use wsp_words::exit::HostError;

use wsp_core::types::pending::{Clock, DaemonChannel, DaemonReach, DeviceDoor, Store};
use wsp_core::types::wiring::{HereDaemon, PlaceBackends};

use super::helpers::LinkSocket;
use super::pane_ports::PanePorts;

/// Every place door answer: the TS throws a `HostError` (spec 05) or answers.
pub type PlaceResult<T> = Result<T, HostError>;

/// One document per joined computer, keyed by the id this host knows it by (types.ts:60).
pub const PLACES: &str = "places";
/// One document per place a person set anything on, keyed by place id, holding only what they set (types.ts:62).
pub const CAPS: &str = "caps";
/// The one document naming which place a verb means when nobody says (types.ts:64-65).
pub const DEFAULT_COLLECTION: &str = "place-default";
pub const DEFAULT_ID: &str = "default";

/// What the store keeps about a joined computer (types.ts:69-100). The key is the whole of its identity. Field order
/// is the TS interface's; the JSON a record is written as keeps the order its writer built it in (spec types.md R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRecord {
    pub id: String,
    pub name: String,
    pub public_key: String,
    pub joined_at: String,
    pub last_seen_at: String,
    pub report: PlaceReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_facts: Option<BackendFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<PlaceRecordRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialled: Option<PlaceDialled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<PlaceSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<HeldApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picks: Option<RecipeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_at: Option<String>,
}

/// The road as the record keeps it (types.ts:104-109): what a client is told (`ssh`, `from`, `back`) plus the key
/// file the person named at the add and the host key the box's ssh answered with, neither of which a client reads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRecordRoad {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<PlaceBack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
}

impl PlaceRecordRoad {
    /// The road as a client may read it: `ssh`, `from` and `back` alone, field by field (views.ts:297-299).
    pub fn view(&self) -> PlaceRoad {
        todo!()
    }
}

/// A row as the record keeps it (types.ts:119): a folder's row carries the pick its project was made from and when
/// that project was made, which no answer carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldRow {
    #[serde(flatten)]
    pub row: PlaceProvisionRow,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<RecipeFileFoldersValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// `Omit<PlaceApplied, "rows"> & { rows: HeldRow[] }` (types.ts:120).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldApplied {
    pub hash: String,
    pub at: String,
    pub rows: Vec<HeldRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<IndexMap<String, String>>,
}

/// The project an earlier setup made from a folder and the pick it was made from (types.ts:123).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderMove {
    pub id: String,
    pub pick: RecipeFileFoldersValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// The kinds `made_by_setup` reads (types.ts:113 takes only "plugins").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MadeKind {
    Plugins,
}

/// `madeBySetup(held, kind)` (types.ts:113-114): the keys of `held.picks[kind]`, in the picks' order, whose applied
/// row `<kind>/<key>` reads `installed`. A row found already there (`present`) is never one.
pub fn made_by_setup(held: &PlaceRecord, kind: MadeKind) -> Vec<String> {
    todo!()
}

/// `appliedView(applied)` (types.ts:126): each row less `pick` and `createdAt`.
pub fn applied_view(applied: &HeldApplied) -> PlaceApplied {
    todo!()
}

/// `projectsMade(held)` (types.ts:131): the ids of `row.project` over the applied rows that name one, in row order.
pub fn projects_made(held: &PlaceRecord) -> IndexSet<String> {
    todo!()
}

/// `isPlaceRecord(v)` (types.ts:133-136): an object whose `id`, `publicKey` and `name` are strings. Nothing else is
/// checked; `place_record_of` reads such a value as a record, `None` where it is not one or does not deserialize.
pub fn is_place_record(v: &serde_json::Value) -> bool {
    todo!()
}

/// The value a store document reads as, where `is_place_record` holds and the rest deserializes.
pub fn place_record_of(v: &serde_json::Value) -> Option<PlaceRecord> {
    todo!()
}

/// What this computer is, as a row of the same list (types.ts:144-153).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HerePlace {
    pub name: String,
    pub label: Option<String>,
    pub mac: Option<MacKind>,
    pub os: Option<String>,
    pub shape: Option<WorkspaceSize>,
    pub disk_free_bytes: Option<u64>,
    pub home: Option<String>,
}

/// The provider the wiring forks on, as `wiring.provider()` answers it (types.ts:162).
#[derive(Clone, Debug, PartialEq)]
pub struct WiredProvider {
    pub id: String,
    pub rate_usd_per_hour: f64,
}

/// One login over ssh as this host holds it (types.ts:264-269).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceLogin {
    pub ssh: String,
    pub key_path: Option<String>,
    pub host_key: Option<String>,
}

/// What one update is told (types.ts:284-295).
#[derive(Clone)]
pub struct PlaceUpdateRequest {
    pub place_id: String,
    pub name: String,
    pub report: PlaceReport,
    pub daemon: bool,
    pub link: Option<Rc<dyn DaemonReach>>,
    pub ssh: Option<PlaceLogin>,
    pub sudo_password: Option<String>,
}

/// Which road carried an update's binary (types.ts:300).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRoad {
    Link,
    Ssh,
}

/// What an update answers (types.ts:299-303).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceUpdateLanded {
    pub road: UpdateRoad,
    pub at: String,
    pub kept: Option<String>,
}

/// What one leave over the ssh road is told (types.ts:312-324).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceLeaveRequest {
    pub place_id: String,
    pub name: String,
    pub report: PlaceReport,
    pub ssh: PlaceLogin,
    pub sudo_password: Option<String>,
    pub force: Option<bool>,
    pub projects: Option<Vec<String>>,
}

/// Written before anything of wsp's is sent (types.ts:358): the undo script, the login, the key ssh answered with.
#[async_trait(?Send)]
pub trait BeforeDeploy {
    async fn before_deploy(&self, undo: String, ssh: String, host_key: Option<String>) -> PlaceResult<()>;
}

/// What one install is told (types.ts:334-359). `code` is the whole join token (`join_token(code, fingerprint)`).
#[derive(Clone)]
pub struct PlaceInstallRequest {
    pub address: String,
    pub name: Option<String>,
    pub ssh_port: Option<u16>,
    pub key_path: Option<String>,
    pub host_key: Option<String>,
    pub sudo_password: Option<String>,
    pub code: String,
    pub host_urls: Vec<String>,
    pub door_port: Option<u16>,
    pub relay: Option<String>,
    pub held: Option<Vec<String>>,
    pub before_deploy: Option<Rc<dyn BeforeDeploy>>,
}

/// What the install answers once the computer has run its own join (types.ts:363-375).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceInstalled {
    pub name: String,
    pub host_key: Option<String>,
    pub ssh: Option<String>,
    pub ssh_key_path: Option<String>,
    pub back: Option<PlaceBack>,
}

/// The state a stage of an install is said in (types.ts:379).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Running,
    Done,
    Failed,
}

/// `PlaceStaging` (types.ts:379): how far one install has got. `ms` is how long the step took, said as it ends.
pub type PlaceStaging = Rc<dyn Fn(PlaceAddStep, StageState, Option<String>, Option<String>, Option<u64>)>;

/// `PlaceInstaller` (types.ts:380).
#[async_trait(?Send)]
pub trait PlaceInstaller {
    async fn install(&self, req: PlaceInstallRequest, stage: PlaceStaging) -> PlaceResult<PlaceInstalled>;
}

/// `PlaceDialler` (types.ts:274): one dial over the login; throws the road's own sentence.
#[async_trait(?Send)]
pub trait PlaceDialler {
    async fn dial(&self, login: &PlaceLogin) -> PlaceResult<()>;
}

/// `PlaceLogReader` (types.ts:278): the last lines the agent wrote there.
#[async_trait(?Send)]
pub trait PlaceLogReader {
    async fn log(&self, login: &PlaceLogin) -> PlaceResult<Vec<String>>;
}

/// `PlaceUpdater` (types.ts:308): `None` where no binary was asked for.
#[async_trait(?Send)]
pub trait PlaceUpdater {
    async fn update(&self, req: PlaceUpdateRequest) -> PlaceResult<Option<PlaceUpdateLanded>>;
}

/// `PlaceLeaver` (types.ts:329): the lines the leave said it took.
#[async_trait(?Send)]
pub trait PlaceLeaver {
    async fn leave(&self, req: PlaceLeaveRequest) -> PlaceResult<Vec<String>>;
}

/// `wiring.forgetKey(placeId)` (types.ts:187): the sentence naming where the key still is, or `None`.
#[async_trait(?Send)]
pub trait PlaceKeyForgetter {
    async fn forget_key(&self, place_id: &str) -> PlaceResult<Option<String>>;
}

/// The verb a sudo is read for (types.ts:192).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SudoVerb {
    Remove,
    Update,
}

/// `wiring.sudoOver(login, password, act)` (types.ts:192): answers the road word, `"taken"` where the typed password
/// was tried and taken; throws the add's own refusal (a host key changed included, `is_place_host_key_changed`).
#[async_trait(?Send)]
pub trait PlaceSudo {
    async fn sudo_over(&self, login: &PlaceLogin, sudo_password: Option<&str>, verb: SudoVerb, name: &str) -> PlaceResult<String>;
}

/// `wiring.githubToken()` (types.ts:198).
#[async_trait(?Send)]
pub trait PlaceGithubToken {
    async fn github_token(&self) -> PlaceResult<()>;
}

/// `wiring.undo(login, script)` (types.ts:201).
#[async_trait(?Send)]
pub trait PlaceUndoer {
    async fn undo(&self, login: &PlaceLogin, script: &str) -> PlaceResult<()>;
}

/// What a script run over a login answered (types.ts:204).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOver {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

/// `wiring.runOver(login, script, timeoutMs, sudoPassword)` (types.ts:204).
#[async_trait(?Send)]
pub trait PlaceRunOver {
    async fn run_over(&self, login: &PlaceLogin, script: &str, timeout_ms: u64, sudo_password: Option<&str>) -> PlaceResult<RunOver>;
}

/// Where a forward's place file is read (types.ts:217): the home there, and the computer it must name.
#[derive(Clone, Debug, PartialEq)]
pub struct BackOn {
    pub home: String,
    pub computer: Option<BackComputer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackComputer {
    pub id: String,
    pub name: String,
}

/// The port the door stands on, asked each time (types.ts:221).
pub type DoorAt = Rc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<u16>>>>>;

/// `PlaceBackHolder` (types.ts:211-223).
#[async_trait(?Send)]
pub trait PlaceBackHolder {
    async fn hold(&self, login: &PlaceLogin, back: PlaceBack, on: BackOn, moved: Option<Rc<dyn Fn(PlaceBack)>>) -> PlaceResult<PlaceBack>;
    fn release(&self, login: &PlaceLogin);
    fn door(&self, at: DoorAt);
    fn close(&self);
}

/// `on` of a setup plan (types.ts:234).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupOn {
    pub home: String,
    pub stores: Option<IndexMap<String, String>>,
    pub recipe: Option<String>,
}

/// One row taken out of a recipe (types.ts:239).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedRow {
    pub kind: String,
    pub name: String,
}

/// What some picks weigh (types.ts:244).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicksWeight {
    pub bytes: u64,
    pub unmeasured: u64,
}

/// `PlaceProvisioner` (types.ts:228-245). The optional methods answer `None` where the TS provisioner lacks them;
/// `has_undo` and `has_estimate` say whether they exist, since a missing one changes the road (manage.ts:816).
#[async_trait(?Send)]
pub trait PlaceProvisioner {
    async fn setup(&self, picks: &RecipeFile, on: SetupOn, only: Option<&IndexSet<PlaceSetupStep>>) -> PlaceResult<ProvisionPlan>;
    async fn floor(&self, machine: Rc<dyn Machine>, home: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>>;
    async fn step(&self, machine: Rc<dyn Machine>, plan: &ProvisionPlan, step: &EngineStep, run: &SetupRun, stage: ProvisionStage, on: &ProvisionOn) -> PlaceResult<Vec<PlaceProvisionRow>>;
    fn has_undo(&self) -> bool;
    async fn undo(&self, before: &RecipeFile, removed: &[RemovedRow], home: &str, stores: Option<&IndexMap<String, String>>) -> PlaceResult<Vec<PlaceUndo>>;
    fn has_project_servers(&self) -> bool;
    async fn project_servers(&self, machine: Rc<dyn Machine>, picks: &RecipeFile, key: &str, path: &str, stage: ProvisionStage, on: &ProvisionOn, recipe: Option<&str>) -> PlaceResult<Vec<PlaceProvisionRow>>;
    fn has_estimate(&self) -> bool;
    async fn estimate(&self, picks: &RecipeFile) -> PlaceResult<PicksWeight>;
}

/// Taking one row off a computer (types.ts:250-260).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceUndo {
    pub key: String,
    pub label: String,
    pub ids: Vec<String>,
    pub owner: Option<String>,
    pub cmd: Option<String>,
    pub dests: Option<Vec<String>>,
    pub folder: Option<String>,
    pub note: Option<String>,
}

/// What a host wires for the places it holds (types.ts:157-208). Absent wiring is a runtime that serves no place.
#[derive(Clone)]
pub struct PlaceWiring {
    pub host_key: PlaceKeyPair,
    pub provider: Option<Rc<dyn Fn() -> Option<WiredProvider>>>,
    pub here: Rc<dyn Fn() -> HerePlace>,
    pub host_name: Rc<dyn Fn() -> String>,
    pub install: Option<Rc<dyn PlaceInstaller>>,
    pub dial: Option<Rc<dyn PlaceDialler>>,
    pub log: Option<Rc<dyn PlaceLogReader>>,
    pub update: Option<Rc<dyn PlaceUpdater>>,
    pub leave: Option<Rc<dyn PlaceLeaver>>,
    pub forget_key: Option<Rc<dyn PlaceKeyForgetter>>,
    pub sudo_over: Option<Rc<dyn PlaceSudo>>,
    pub provision: Option<Rc<dyn PlaceProvisioner>>,
    pub github_token: Option<Rc<dyn PlaceGithubToken>>,
    pub undo: Option<Rc<dyn PlaceUndoer>>,
    pub run_over: Option<Rc<dyn PlaceRunOver>>,
    pub back: Option<Rc<dyn PlaceBackHolder>>,
}

/// One image snapshot this host saved at a provider (types.ts:384-387).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldImage {
    pub id: String,
    pub template_id: Option<String>,
}

/// A project recorded on a place (types.ts:394).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectOn {
    pub id: String,
    pub name: String,
    pub path: String,
    pub checkout: Option<String>,
}

/// A row's id and kind, as `runningOn` places a workspace by (types.ts:406).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowId {
    pub id: String,
    pub kind: wsp_wire::views::place_kind::PlaceKind,
}

/// A provider's spend since midnight (types.ts:425).
#[derive(Clone, Debug, PartialEq)]
pub struct SpentToday {
    pub place: String,
    pub today_usd: f64,
}

/// What a sign-in or a mint started answers (types.ts:428,432).
pub struct SignInHandle {
    pub leave: Box<dyn Fn()>,
    pub stop: Option<Box<dyn Fn()>>,
}

pub type SignInEmit = Rc<dyn Fn(AgentsSignInEvent)>;

/// The one road into the runtime a place needs (types.ts:391-454). A method documented "absent" answers `None` (or
/// `false`) where the TS method is missing; the door reads that as the TS reads a missing method.
#[async_trait(?Send)]
pub trait PlaceRecording {
    async fn projects_on(&self, place_id: &str) -> PlaceResult<Vec<ProjectOn>>;
    async fn holds_on(&self, place_id: &str, read: bool) -> PlaceResult<PlaceHolds>;
    async fn drop_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook>;
    async fn forget_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook>;
    async fn running_on(&self, place_id: &str, places: &[RowId]) -> PlaceResult<u64>;
    /// Absent: `None`.
    async fn machines_on(&self, place_id: &str) -> Option<PlaceResult<Vec<String>>>;
    /// Absent: `None`.
    async fn forget_records_on(&self, place_id: &str) -> Option<PlaceResult<()>>;
    /// Absent: `None`.
    async fn sub_holds_on(&self, place_id: &str) -> Option<PlaceResult<Vec<String>>>;
    /// Absent: `None`.
    async fn release_records_on(&self, place_id: &str) -> Option<PlaceResult<()>>;
    /// Absent (with `drop_image_on`): `has_images` false.
    fn has_images(&self) -> bool;
    async fn images_on(&self, place_id: &str) -> PlaceResult<Vec<HeldImage>>;
    async fn drop_image_on(&self, place_id: &str, id: &str) -> PlaceResult<()>;
    /// Absent: `None`.
    async fn spent_today(&self, places: &[PlaceView]) -> Option<PlaceResult<Vec<SpentToday>>>;
    /// Absent: `None`.
    async fn sign_in(&self, place_id: &str, agent: &str, emit: SignInEmit) -> Option<PlaceResult<SignInHandle>>;
    /// Absent: `None`.
    async fn mint_here(&self, agent: &str, emit: SignInEmit) -> Option<PlaceResult<SignInHandle>>;
    /// Absent: `None`.
    async fn sign_in_line(&self, place_id: &str, agent: &str) -> Option<PlaceResult<SignInLine>>;
    /// Absent: `None`.
    fn stores_on(&self, place_id: &str, home: &str) -> Option<IndexMap<String, String>>;
    /// Absent: `None`; present and holding no login: `Some(Ok(None))`.
    async fn login_here(&self, agent: &str) -> Option<PlaceResult<Option<Vec<u8>>>>;
    /// Absent: `None`.
    async fn add_folder(&self, place_id: &str, key: &str, folder: &RecipeFileFoldersValue, mv: Option<&FolderMove>, stage: Option<Rc<dyn Fn(&str)>>) -> Option<PlaceResult<HeldRow>>;
    /// Absent: `None`.
    async fn folder_remote(&self, folder: &RecipeFileFoldersValue) -> Option<PlaceResult<Option<String>>>;
    async fn remove_folder(&self, place_id: &str, project_id: &str) -> PlaceResult<()>;
    async fn folder_look(&self, project_id: &str, was: &RecipeFileFoldersValue, now: &RecipeFileFoldersValue) -> PlaceResult<()>;
}

/// A recipe resolved against this computer now (types.ts:514-517).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub file: RecipeFile,
    pub items: IndexMap<String, String>,
    pub hash: String,
}

#[async_trait(?Send)]
pub trait RecipeResolver {
    async fn resolve(&self, slug: &str) -> PlaceResult<Resolved>;
    async fn resolve_file(&self, file: &RecipeFile) -> PlaceResult<Resolved>;
}

/// What a place's row says about its copy of the image while it is not standing (types.ts:475).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyBuild {
    pub line: String,
    pub stopped: bool,
}

/// `KillConfirm` (engine golden.ts:35), held here until golden.ts is ported (#2441's ruling): how long a delete is
/// read back for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KillConfirm {
    pub grace_ms: Option<u64>,
    pub poll_ms: Option<u64>,
}

/// What `onSetup` hears (types.ts:471).
#[derive(Clone, Debug, PartialEq)]
pub enum SetupEvent {
    Setup(wsp_wire::views::place::PlaceSetupEvent),
    Pending(wsp_wire::views::place_more::PlacePendingEvent),
    Sync(wsp_wire::views::place::PlaceSyncEvent),
    RecipesChanged(wsp_wire::views::place::RecipesChangedEvent),
}

/// What a client watching the places hears (types.ts:140, the protocol's four shapes).
#[derive(Clone, Debug, PartialEq)]
pub enum PlaceEvent {
    Joined(wsp_wire::views::place_more::PlaceJoinedEvent),
    Changed(wsp_wire::views::place_more::PlaceChangedEvent),
    Removed(wsp_wire::views::place_more::PlaceRemovedEvent),
    Absent(wsp_wire::views::place_more::PlaceAbsentEvent),
}

/// `PlaceDoorOptions` (types.ts:456-511). `None` is the TS's absent option; the context reads each default.
#[derive(Clone)]
pub struct PlaceDoorOptions {
    pub store: Rc<dyn Store>,
    pub devices: Rc<dyn DeviceDoor>,
    pub wiring: PlaceWiring,
    pub recording: Rc<dyn PlaceRecording>,
    pub providers: Option<Rc<dyn Fn() -> Rc<dyn PlaceBackends>>>,
    pub on_daemon_event: Option<Rc<dyn Fn(&str, &DaemonEvent)>>,
    pub on_stage: Option<Rc<dyn Fn(&PlaceStageEvent)>>,
    pub on_setup: Option<Rc<dyn Fn(&SetupEvent)>>,
    pub copy_build: Option<Rc<dyn Fn(&str) -> Option<CopyBuild>>>,
    pub nap_ms: Option<u64>,
    pub here_daemon: Option<Rc<dyn HereDaemon>>,
    pub nap_changed: Option<Rc<dyn Fn(&str)>>,
    pub join_wait_ms: Option<u64>,
    pub update_wait_ms: Option<u64>,
    pub dial_wait_ms: Option<u64>,
    pub frame_wait_ms: Option<u64>,
    pub relink_wait_ms: Option<u64>,
    pub seen_every_ms: Option<u64>,
    pub vault: Option<Rc<dyn Fn() -> IndexMap<String, String>>>,
    pub recipes: Option<Rc<dyn Fn() -> Option<Rc<dyn RecipeResolver>>>>,
    pub owner: Option<Rc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = String>>>>>,
    pub kill_confirm: Option<KillConfirm>,
    /// `opts.now` and `opts.schedule` (types.ts:508-510), one clock (spec 03 Proposal 2b).
    pub clock: Option<Rc<dyn Clock>>,
}

/// The host's half of one handshake (types.ts:521-528).
pub struct PlaceChallenge {
    pub nonce: String,
    pub host_public_key: String,
    pub signature: String,
    pub ephemeral: String,
    pub expect: Vec<u8>,
    pub seal: Seal,
}

/// What `join` answers (types.ts:538).
pub struct JoinAnswer {
    pub reply: PlaceJoinReply,
    pub expect: Vec<u8>,
    pub seal: Seal,
    pub notice: Option<String>,
}

/// What `auth` answers (types.ts:543).
pub enum AuthAnswer {
    Reply { reply: PlaceAuthReply, expect: Vec<u8>, seal: Seal },
    Refusal { refusal: String, signed: PlaceAuthRefusal },
}

/// A device token a join's prove answered (types.ts:551).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvedDevice {
    pub device_id: String,
    pub device_token: String,
}

/// What `prove` and the join's half of it answer (types.ts:551, views.ts:330).
#[derive(Clone, Debug, PartialEq)]
pub enum Proved {
    Report { report: PlaceReport, device: Option<ProvedDevice> },
    Refusal { refusal: String },
}

/// A computer the person joined as the threads in a folder run on (types.ts:576).
pub struct FolderComputer {
    pub machine: std::sync::Arc<PlaceFolderMachine>,
    pub home: String,
    pub cpu: u32,
    pub mem_mb: u64,
    pub tools: bool,
}

/// The pane a forward opens a port for (types.ts:636).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardAsk {
    pub pane: Option<(String, String)>,
    pub standing: bool,
}

/// One command on a place over its link (types.ts:677).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceExecOpts {
    pub timeout_ms: Option<u64>,
    pub stdin: Option<Vec<u8>>,
}

/// What `add` is asked (types.ts:651).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AddAsk {
    pub add_id: Option<String>,
    pub address: String,
    pub name: Option<String>,
    pub ssh_port: Option<u16>,
    pub key_path: Option<String>,
    pub host_key: Option<String>,
    pub sudo_password: Option<String>,
    pub host_urls: Vec<String>,
    pub door_port: Option<u16>,
    pub relay: Option<String>,
    pub choices: Option<RecipeFile>,
    pub recipe: Option<String>,
}

/// What `setUp` is asked (types.ts:657).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetUpAsk {
    pub choices: Option<RecipeFile>,
    pub recipe: Option<String>,
    pub add_id: Option<String>,
    pub row: Option<String>,
}

/// What `update` and `remove` are asked (types.ts:691,697).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoveAsk {
    pub sudo_password: Option<String>,
    pub force: Option<bool>,
    pub forget: Option<bool>,
}

/// What a place word names (types.ts:641): `None` is this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceFor {
    pub place_id: Option<String>,
}

/// What an install answers once the computer has dialled in (types.ts:793-799).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceAdded {
    pub add_id: String,
    pub place: PlaceView,
    pub host_key: Option<String>,
    pub said: Option<String>,
    pub pending: Option<PendingComputer>,
}

/// What a set up answers (types.ts:802-807).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceSetUp {
    pub add_id: String,
    pub place: PlaceView,
    pub setup: Option<PlaceSetup>,
    pub said: Option<String>,
}

/// The host's side of the place link (types.ts:531-725). Each method's contract is its TS doc comment there.
#[async_trait(?Send)]
pub trait PlaceDoor {
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge>;
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>>;
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<AuthAnswer>;
    fn host_key(&self) -> String;
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved>;
    async fn attach(&self, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()>;
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>>;
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>) -> Option<Rc<dyn DaemonChannel>>;
    async fn watch_sys(&self, place_id: &str, f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>>;
    async fn load(&self) -> PlaceResult<()>;
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn MachineBackend>>;
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer>;
    fn joined(&self, place_id: &str) -> bool;
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<std::sync::Arc<dyn MachineBackend>>;
    fn name_of(&self, place_id: &str) -> String;
    fn settings_at(&self, place_id: &str) -> PlaceSettings;
    /// `None` no row; `Some(None)` no limit.
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>>;
    fn threads_at(&self, place_id: &str) -> Option<u64>;
    fn machines_at(&self, place_id: &str) -> Option<u64>;
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>>;
    fn github_from_vault(&self, place_id: &str) -> bool;
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String>;
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()>;
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()>;
    async fn key_landed(&self, agent: &str) -> PlaceResult<()>;
    async fn gh_there(&self, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>>;
    fn offer_of(&self, place_id: &str) -> Option<String>;
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16>;
    fn pane_forwards(&self) -> Rc<PanePorts>;
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor>;
    async fn default_place(&self) -> PlaceResult<PlaceFor>;
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()>;
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()>;
    async fn add(&self, req: AddAsk, now: u64) -> PlaceResult<PlaceAdded>;
    async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp>;
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>>;
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer>;
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial>;
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView>;
    async fn road(&self, place_id: &str) -> PlaceResult<u16>;
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult>;
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>>;
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>>;
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>>;
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>>;
    fn adds(&self) -> Vec<PlaceAddJob>;
    async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply>;
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds>;
    async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved>;
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>>;
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView>;
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>>;
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>>;
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView>;
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>>;
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate>;
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>>;
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>>;
    /// Answers the id `off` takes.
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64;
    fn off(&self, id: u64);
    async fn close(&self) -> PlaceResult<()>;
}

/// The one refusal for a runtime served without places wired (types.ts:729).
pub const NO_PLACE_DOOR: &str = "this runtime holds no places; the host that serves the app wires them";

/// The refusal an update gets on a runtime wired with no updater (types.ts:732).
pub const NO_PLACE_UPDATER: &str = "this runtime carries no daemon to put on a computer; the host that serves the app wires one";

/// `placeUpdateSlowLine(name, seconds)` (types.ts:736-737).
pub fn place_update_slow_line(name: &str, seconds: u64) -> String {
    todo!()
}

/// `placeSweptOverSshLine(name)` (types.ts:741).
pub fn place_swept_over_ssh_line(name: &str) -> String {
    todo!()
}

/// `placeLoginRoadLine(name, at, said)` (types.ts:747-748).
pub fn place_login_road_line(name: &str, at: &str, said: Option<&str>) -> String {
    todo!()
}

/// `placeSweptOverLinkLine(name, at, said)` (types.ts:753-754).
pub fn place_swept_over_link_line(name: &str, at: &str, said: Option<&str>) -> String {
    todo!()
}

/// `placeElsewhereSweptOverLinkLine(name, away)` (types.ts:758-759).
pub fn place_elsewhere_swept_over_link_line(name: &str, away: &str) -> String {
    todo!()
}

/// The marks the TS error classes become (types.ts:764-787): a `HostError` carrying the mark is an instance of the
/// class. `place_login_refused` also stamps kind `PLACE_LOGIN_REFUSED_KIND`, `place_provisioning` kind conflict.
pub const PLACE_HOST_KEY_CHANGED_MARK: &str = "placeHostKeyChanged";
pub const PLACE_LOGIN_REFUSED_MARK: &str = "placeLoginRefused";
pub const PLACE_FORKS_NOWHERE_MARK: &str = "placeForksNowhere";
pub const PLACE_ADD_TAKEN_BACK_MARK: &str = "placeAddTakenBack";
pub const PLACE_PROVISIONING_MARK: &str = "placeProvisioning";

/// `new PlaceHostKeyChangedError(message)` (types.ts:764): no kind.
pub fn place_host_key_changed(message: &str) -> HostError {
    todo!()
}
pub fn is_place_host_key_changed(e: &HostError) -> bool {
    todo!()
}

/// `new PlaceLoginRefusedError(message)` (types.ts:769-771): kind `PLACE_LOGIN_REFUSED_KIND`.
pub fn place_login_refused(message: &str) -> HostError {
    todo!()
}
pub fn is_place_login_refused(e: &HostError) -> bool {
    todo!()
}

/// `new PlaceForksNowhereError(message)` (types.ts:775): no kind.
pub fn place_forks_nowhere(message: &str) -> HostError {
    todo!()
}
pub fn is_place_forks_nowhere(e: &HostError) -> bool {
    todo!()
}

/// `new PlaceAddTakenBackError(message)` (types.ts:779): no kind.
pub fn place_add_taken_back(message: &str) -> HostError {
    todo!()
}
pub fn is_place_add_taken_back(e: &HostError) -> bool {
    todo!()
}

/// `new PlaceProvisioningError(message)` (types.ts:785-787): kind conflict.
pub fn place_provisioning(message: &str) -> HostError {
    todo!()
}
pub fn is_place_provisioning(e: &HostError) -> bool {
    todo!()
}
