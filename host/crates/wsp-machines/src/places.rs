// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places.ts` at 65f6b89c7 (the door's assembly, 50 lines) and `places/`. Spec:
//! `spec/places_door.md`.

pub mod context;
pub mod helpers;
pub mod link;
pub mod manage;
pub mod pane_ports;
pub mod provider_remove;
pub mod readings;
pub mod records;
pub mod setup;
pub mod types;
pub mod views;

use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::types::pending::{DaemonChannel, DaemonReach};
use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_engine::provision::ProvisionStage;
use wsp_store::keys::Seal;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetupStep};
use wsp_wire::views::place_more::{PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceUpdateReply, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};

use context::PlaceDoorContext;
use helpers::LinkSocket;
use link::LinkDoor;
use manage::ManageDoor;
use pane_ports::PanePorts;
use types::{
    AddAsk, AuthAnswer, FolderComputer, ForwardAsk, JoinAnswer, PlaceAdded, PlaceChallenge, PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceExecOpts,
    PlaceFor, PlaceRecord, PlaceResult, PlaceSetUp, Proved, RemoveAsk, SetUpAsk,
};
use views::LinkReads;

pub use helpers::{
    place_home_refusal, sign_ins_of, taken_report, vault_sign_in, ADD_NOT_TAKEN_BACK_LINE, ADD_STOPPED_FIX, ADD_STOPPED_LINE, PLACE_BAD_KEY_REFUSAL,
};
pub use types::{
    is_place_add_taken_back, is_place_forks_nowhere, is_place_host_key_changed, is_place_login_refused, is_place_provisioning, is_place_record,
    place_add_taken_back, place_elsewhere_swept_over_link_line, place_forks_nowhere, place_host_key_changed, place_login_refused,
    place_login_road_line, place_provisioning, place_swept_over_link_line, place_swept_over_ssh_line, place_update_slow_line, HerePlace,
    PlaceBackHolder, PlaceDialler, PlaceInstallRequest, PlaceInstalled, PlaceInstaller, PlaceLeaveRequest,
    PlaceLeaver, PlaceLogReader, PlaceLogin, PlaceProvisioner, PlaceRecordRoad, PlaceRecording, PlaceStaging, PlaceUndo, PlaceUpdateLanded,
    PlaceUpdateRequest, PlaceUpdater, PlaceWiring, RecipeResolver, NO_PLACE_DOOR, NO_PLACE_UPDATER, PLACES,
};
pub use wsp_store::keys::{new_place_key_pair, PlaceKeyPair};

/// The methods of the link half (places.ts:46). The TS spreads the manage half over it, so a name in both would be the
/// manage half's; none is.
pub const LINK_HALF: [&str; 33] = ["answer_challenge", "join", "auth", "host_key", "prove", "attach", "link", "channel", "watch_sys", "load", "backend_of", "folder_computer", "joined", "forking_backend", "name_of", "settings_at", "turn_limit_at", "threads_at", "machines_at", "sign_ins_at", "github_from_vault", "vault_at", "login_landed", "logins_again", "key_landed", "gh_there", "offer_of", "forward", "pane_forwards", "place_for", "default_place", "mark_used", "mark_default_if_none"];

/// The methods of the manage half (places.ts:46).
pub const MANAGE_HALF: [&str; 28] = ["add", "set_up", "pending", "choose", "dial", "set", "road", "exec", "report_of", "home_of", "list", "rows", "adds", "update", "holds", "remove", "find", "follow", "recipe_changed", "followers", "skip", "setup_log", "estimate", "unfollow", "picks_of", "on", "off", "close"];

/// What views and manage read off `ctx.door` (views.ts:126,178,236-238,410): a forwarder over the door the context
/// holds, so the areas built before the door still reach its link half.
pub struct DoorLinkReads {
    pub ctx: Rc<PlaceDoorContext>,
}

#[async_trait(?Send)]
impl LinkReads for DoorLinkReads {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        todo!()
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> {
        todo!()
    }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        todo!()
    }
}

/// The door `make_place_door` answers (places.ts:46): the link half and the manage half behind one `PlaceDoor`, each
/// method on the half that owns it.
pub struct AssembledDoor {
    pub ctx: Rc<PlaceDoorContext>,
    pub link: Rc<LinkDoor>,
    pub manage: Rc<ManageDoor>,
}

#[async_trait(?Send)]
impl PlaceDoor for AssembledDoor {
    /// The link half (places.ts:46).
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<AuthAnswer> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn host_key(&self) -> String {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn attach(&self, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>) -> Option<Rc<dyn DaemonChannel>> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn watch_sys(&self, place_id: &str, f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn load(&self) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn MachineBackend>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn joined(&self, place_id: &str) -> bool {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<std::sync::Arc<dyn MachineBackend>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn name_of(&self, place_id: &str) -> String {
        todo!()
    }
    /// The link half (places.ts:46).
    fn settings_at(&self, place_id: &str) -> PlaceSettings {
        todo!()
    }
    /// The link half (places.ts:46).
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn threads_at(&self, place_id: &str) -> Option<u64> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn machines_at(&self, place_id: &str) -> Option<u64> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn github_from_vault(&self, place_id: &str) -> bool {
        todo!()
    }
    /// The link half (places.ts:46).
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn gh_there(&self, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn offer_of(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> {
        todo!()
    }
    /// The link half (places.ts:46).
    fn pane_forwards(&self) -> Rc<PanePorts> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn default_place(&self) -> PlaceResult<PlaceFor> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> {
        todo!()
    }
    /// The link half (places.ts:46).
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn add(&self, req: AddAsk, now: u64) -> PlaceResult<PlaceAdded> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    fn adds(&self) -> Vec<PlaceAddJob> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        todo!()
    }
    /// The manage half (places.ts:46).
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        todo!()
    }
    /// The manage half (places.ts:46).
    fn off(&self, id: u64) {
        todo!()
    }
    /// The manage half (places.ts:46).
    async fn close(&self) -> PlaceResult<()> {
        todo!()
    }
}

/// `makePlaceDoor` (places.ts:38): [`assemble_place_door`], read as the `PlaceDoor` every caller holds.
pub fn make_place_door(opts: PlaceDoorOptions) -> Rc<dyn PlaceDoor> {
    let _ = opts;
    todo!()
}

/// The assembly itself (places.ts:39-46): the context, then the areas in the TS order (records, setup, views, then the
/// link and manage halves), then the door set on the context so an area that reads `ctx.door` finds it. Views and
/// manage are handed [`DoorLinkReads`] for their `LinkReads` and the setup area for `ViewsSetup` and `ManageSetup`.
pub fn assemble_place_door(opts: PlaceDoorOptions) -> Rc<AssembledDoor> {
    let _ = opts;
    todo!()
}
