// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/helpers.ts` at 65f6b89c7: the readers and constants every area of the place door
//! shares. Spec: `spec/places_helpers.md`; `listen_near` is `spec/places_pane_ports.md`'s.

use std::cell::RefCell;
use std::future::Future;
use std::net::SocketAddr;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::{Cancel, Clock};
use wsp_core::place_forward::PlaceForward;
use wsp_core::reach::DaemonReach;
use wsp_core::recipe_sync::RecipeChange;
use wsp_engine::provision::ProvisionPlan;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeSignIn};
use wsp_wire::views::place::{PendingComputer, PlaceApplied, PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetup, PlaceSetupStep, PlaceSync, SetupEnd};
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;
use wsp_words::recipe_file::RecipeOptions;

use super::types::{PlaceInstalled, PlaceWiring};

/// A row landed from outside a run: a [`PlaceProvisionRow`] whose label is optional (helpers.ts:44).
#[derive(Clone, Debug, PartialEq)]
pub struct LandedRow {
    pub row: PlaceProvisionRow,
    /// `None` is the TS row with no `label`; `row.label` is then ignored.
    pub label: Option<String>,
}

/// The rows with `row` landed, or `None` where it lands nothing (helpers.ts:47).
pub fn landed_on(rows: &[PlaceProvisionRow], row: &LandedRow) -> Option<Vec<PlaceProvisionRow>> {
    let _ = (rows, row);
    todo!()
}

/// `work`'s answer, or after `ms` on `clock` the error `"{what} was not answered in {round(ms / 1000)}s"`; the
/// timer is cancelled once `work` settles (helpers.ts:56).
pub async fn bounded<T>(clock: &dyn Clock, work: impl Future<Output = Result<T, HostError>>, ms: u64, what: &str) -> Result<T, HostError> {
    let _ = (clock, work, ms, what);
    todo!()
}

/// Whether a public key off the wire is an ed25519 SPKI DER key in base64 (helpers.ts:81). Never an error.
pub fn reads_as_ed25519(public_key_base64: &str) -> bool {
    let _ = public_key_base64;
    todo!()
}

/// How long a join that has been answered has to send its prove (helpers.ts:91).
pub const JOIN_PROVE_MS: u64 = 120_000;

/// The one sentence a join whose key this host cannot verify against is refused with (helpers.ts:94).
pub const PLACE_BAD_KEY_REFUSAL: &str = "that join sent a key this host cannot verify a signature against; a place's key is ed25519, as SPKI DER in base64";

/// The refusal of a report whose home is no plain path; `None` and `""` read "no home folder" (helpers.ts:99).
pub fn place_home_refusal(said: Option<&str>) -> String {
    let _ = said;
    todo!()
}

/// The one door every report passes (helpers.ts:110): the home held to a plain path, the PATH to its plain folders,
/// a harness's store folder kept only where plain.
pub fn taken_report(report: &PlaceReport) -> Result<PlaceReport, HostError> {
    let _ = report;
    todo!()
}

/// The agents on a computer as this host knows them (helpers.ts:123).
pub fn agents_on(report_agents: &[String], rows: &[PlaceProvisionRow]) -> Vec<String> {
    let _ = (report_agents, rows);
    todo!()
}

/// What stands for each agent on a computer, or `None` where the report carries no logins list (helpers.ts:137).
/// `agents` `None` is `agents_on(report, rows)`.
pub fn sign_ins_of(
    report_agents: &[String],
    report_logins: Option<&[String]>,
    vault: &IndexMap<String, String>,
    rows: &[PlaceProvisionRow],
    agents: Option<&[String]>,
) -> Option<IndexMap<String, AgentSignInState>> {
    let _ = (report_agents, report_logins, vault, rows, agents);
    todo!()
}

/// The file under a computer's logins folder an agent's shared login writes (helpers.ts:160).
pub fn shared_login_file(agent_id: &str) -> Option<String> {
    let _ = agent_id;
    todo!()
}

/// How long the read of which shared logins stand on a computer gets (helpers.ts:166).
pub const LOGINS_READ_MS: u64 = 10_000;

/// The script that prints which shared logins stand under that logins folder (helpers.ts:173).
pub fn shared_logins_script(logins: &str) -> String {
    let _ = logins;
    todo!()
}

/// A logins list with each shared login's file as `found` says it stands now; `None` where that is the list
/// already (helpers.ts:179).
pub fn relisted_logins(listed: &[String], found: &[String]) -> Option<Vec<String>> {
    let _ = (listed, found);
    todo!()
}

/// `vault-key` where this host's vault holds the agent's token or key, else `none` (helpers.ts:186).
pub fn vault_sign_in(agent_id: &str, vault: &IndexMap<String, String>) -> AgentSignInState {
    let _ = (agent_id, vault);
    todo!()
}

/// What a sign-in row says the vault hands an agent's turns on a computer picked `way` (helpers.ts:195).
pub fn vault_held_line(agent_id: &str, vault: &IndexMap<String, String>, way: Option<RecipeSignIn>, here: &str) -> String {
    let _ = (agent_id, vault, way, here);
    todo!()
}

/// The vault as a computer's picks hand it to the turns there (helpers.ts:205): a token dropped for an agent picked
/// to take its key where the key is held.
pub fn picked_vault(vault: &IndexMap<String, String>, picks: Option<&RecipeFile>) -> IndexMap<String, String> {
    let _ = (vault, picks);
    todo!()
}

/// The picks a computer can be set up from, as this host can serve them (helpers.ts:217).
pub fn servable_options(options: &RecipeOptions, vault: &IndexMap<String, String>, held: &IndexSet<String>) -> RecipeOptions {
    let _ = (options, vault, held);
    todo!()
}

/// The agent homes on this computer, as `agents.homesHere` answers them.
pub type HomesHere = Rc<dyn Fn() -> LocalBoxFuture<'static, Result<IndexMap<String, String>, HostError>>>;

/// The login an agent shares into every thread, as this computer holds it; `None` where missing or empty
/// (helpers.ts:224).
pub async fn login_here(homes: HomesHere, agent_id: &str) -> Option<Vec<u8>> {
    let _ = (homes, agent_id);
    todo!()
}

/// How long between writes of a linked place's last seen (helpers.ts:232).
pub const SEEN_EVERY_MS: u64 = 60_000;

/// What a socket is closed with when a newer link for the same place arrives (helpers.ts:236).
pub const REPLACED: &str = "replaced by a newer link";

/// The websocket a place's link rides, as the door uses it (the TS `ws` WebSocket).
pub trait LinkSocket {
    /// `readyState === OPEN`: a socket that is closing is not open.
    fn is_open(&self) -> bool;
    fn close(&self, code: u16, reason: &str);
    /// `once("close")`: called once with the close's code and reason. Answers an id for [`LinkSocket::off_close`].
    fn on_close(&self, f: Box<dyn FnOnce(u16, String)>) -> u64;
    fn off_close(&self, id: u64);
}

/// The forward a place's panes ride, opened at the first pane that asks (helpers.ts:243).
pub type HeldForward = Shared<LocalBoxFuture<'static, Result<Rc<PlaceForward>, HostError>>>;

/// One linked place as the door holds it (helpers.ts:238).
pub struct Live {
    pub socket: Rc<dyn LinkSocket>,
    pub reach: Rc<dyn DaemonReach>,
    /// The last-seen timer; cancelled when the link goes.
    pub seen: Cancel,
    pub forward: Option<HeldForward>,
}

/// One connection riding a forward: what the views write a tunnel's bytes into.
pub trait ForwardConn {
    fn write(&self, bytes: &[u8]);
    fn end(&self);
    fn destroy(&self);
}

/// A loopback listener a forward holds; [`LoopbackListener::close`] frees its port.
pub struct LoopbackListener {
    pub addr: SocketAddr,
    pub(crate) close: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl LoopbackListener {
    /// Stops accepting and frees the port; a second close does nothing.
    pub fn close(&self) {
        todo!()
    }
}

/// One port on this computer carried to one port on a place (helpers.ts:248).
pub struct Forward {
    pub server: Rc<LoopbackListener>,
    pub local_port: u16,
    /// The connections riding it now, by the tunnel id the far side knows each by.
    pub conns: Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>>,
    /// Takes away what a port opened for a pane holds beyond its first listener.
    pub stop: Option<Rc<dyn Fn()>>,
}

/// A connection accepted on a loopback listener.
pub type OnConn = Rc<dyn Fn(tokio::net::TcpStream)>;

/// Listens on `port` on both loopback families, or on the first free one up to 20 above it (helpers.ts:262).
pub async fn listen_near(port: u16, on_conn: OnConn) -> Result<(Vec<Rc<LoopbackListener>>, u16), HostError> {
    let _ = (port, on_conn);
    todo!()
}

/// How long a machine frame waits for its answer when the caller named no bound (helpers.ts:286).
pub const LINK_FRAME_MS: u64 = 300_000;
/// How long a request the host may ask again waits for the computer to open a socket again (helpers.ts:291).
pub const RELINK_WAIT_MS: u64 = 120_000;
/// How long a place gets to say what its backend is (helpers.ts:295).
pub const BACKEND_FACTS_MS: u64 = 10_000;
/// How long an install waits for that answer before it answers the person (helpers.ts:298).
pub const ADD_FACTS_MS: u64 = 2_000;
pub const CAPACITY_MS: u64 = 5_000;
/// How long one dial of a computer gets (helpers.ts:302).
pub const DIAL_MS: u64 = 20_000;
/// How long a computer has to dial back after its own join wrote its place file (helpers.ts:306).
pub const JOIN_WAIT_MS: u64 = 90_000;
/// Finished adds kept beside the running ones (helpers.ts:309).
pub const ADDS_KEPT: usize = 20;
pub const ADD_RUNNING_LINE: &str = "an add under that id is still running";
pub const ADD_RUNNING_FIX: &str = "Leave the id out, or name the add afresh.";

/// What the box itself said while that wait ran out, where a log road and its ssh login exist (helpers.ts:318).
pub async fn box_said(wiring: &PlaceWiring, installed: &PlaceInstalled) -> Vec<String> {
    let _ = (wiring, installed);
    todo!()
}

/// One document per add that has not reached Set up (helpers.ts:325).
pub const PENDING: &str = "pending-computers";

/// A pending add as the store keeps it (helpers.ts:330): the client's view and the door's own fields.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRecord {
    #[serde(flatten)]
    pub view: PendingComputer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

/// A pending add as a client reads it (helpers.ts:333).
pub fn pending_view(p: &PendingRecord) -> PendingComputer {
    let _ = p;
    todo!()
}

pub const ADD_STOPPED_LINE: &str = "the host stopped while wsp was being installed there; what the install put there was taken back";
pub const ADD_STOPPED_FIX: &str = "Add it again.";
pub const ADD_NOT_TAKEN_BACK_LINE: &str = "the host stopped while wsp was being installed there, and what the install put there could not be taken back";

/// One sync of a computer to the recipe it follows (helpers.ts:345).
#[derive(Clone, Debug)]
pub struct SyncJob {
    pub file: RecipeFile,
    pub items: IndexMap<String, String>,
    pub hash: String,
    pub changes: Vec<RecipeChange>,
    pub steps: IndexSet<PlaceSetupStep>,
    /// The rows added or changed, by `<kind>/<row>`.
    pub moved: IndexSet<String>,
    pub before: RecipeFile,
    pub state: PlaceSync,
}

/// How a setup came out (helpers.ts:359).
pub fn setup_outcome(setup: &PlaceSetup, applied: Option<&PlaceApplied>) -> (SetupEnd, Option<String>) {
    let _ = (setup, applied);
    todo!()
}

/// The refusal an estimate gets on a runtime whose provisioner weighs nothing (helpers.ts:367).
pub const NO_ESTIMATE_LINE: &str = "this runtime weighs no picks; the host that serves the app wires the reader";
pub const LOG_READ_MS: u64 = 10_000;
pub const UNDO_MS: u64 = 300_000;
pub const SIGN_IN_SLACK_MS: u64 = 5_000;
pub const GITHUB_MS: u64 = 30_000;

/// The plan with the CLIs `ids` first, each behind the steps it needs (helpers.ts:382).
pub fn cli_first(plan: &ProvisionPlan, ids: &[String]) -> ProvisionPlan {
    let _ = (plan, ids);
    todo!()
}

/// A setup row the engine read off that computer (helpers.ts:400).
pub fn engine_row(r: &PlaceProvisionRow) -> bool {
    let _ = r;
    todo!()
}

pub const INSTALLS: &str = "installs";
pub const FILES: &str = "files";
pub const PROBE_MS: u64 = 30_000;
pub const SIGNIN_STATUS_MS: u64 = 30_000;

/// The message up to its first newline (helpers.ts:410).
pub fn first_line_of(e: &HostError) -> String {
    let _ = e;
    todo!()
}

/// The hash of what a computer was set up with (helpers.ts:413).
pub fn picks_hash(picks: &RecipeFile) -> String {
    let _ = picks;
    todo!()
}

pub const PROVISION_LOG_EVERY_MS: u64 = 2_000;
pub const PROVISION_LOG_LINES: usize = 50;
pub const UPDATE_WAIT_MS: u64 = 60_000;
pub const UPDATE_POLL_MS: u64 = 500;
pub const UNMERGE_MS: u64 = 60_000;

/// The outcome words a row is read by (`installed`, `present`, `skipped`, `failed`), re-exported for the areas.
pub type Outcome = PlaceProvisionRowOutcome;
