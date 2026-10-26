// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve.ts at 65f6b89c7: the protocol server over the websocket. This file holds
//! the options, the host's doors and the listener (spec/serve_wire.md); the door every socket passes, the switch
//! and the ops live in the modules below, one unit each (spec/README.md).
//!
//! The auth model: the long-lived host token travels only in an `auth` frame on an open socket, never in a URL; a
//! computer that is not this one redeems a one-time pairing code as its first frame and holds a device token of its
//! own afterwards; anything else that needs to open a new socket uses a 5-minute single-use ticket minted over an
//! authed socket (`ticket.issue`) and redeemed as `?ticket=...` on the next connect.
pub mod answer;
pub mod door;
pub mod flat;
pub mod listener;
pub mod ops;
pub mod places;
pub mod served;
pub mod socket;
pub mod workspaces;

use std::net::SocketAddr;
use std::rc::Rc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use wsp_core::devices::{DeviceDoor, HeldDevice};
use wsp_core::types::wiring::{HostEditor, HostFolders, HostSsh, HostTerminalConfig, InitDoor, ProjectBundler, ProjectLander, RecipeShelf};
use wsp_wire::types::app_ports::LOOPBACK;
use wsp_wire::types::release::{ReleaseChangedEvent, ReleaseView};
use wsp_wire::views::harness::StartResult;
use wsp_wire::views::place::DoctorLineEvent;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::WorkspaceOut;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::devices::{AccountDevice, AccountView};
use wsp_wire::wire::place_link::{PlaceDoorView, SshHostSuggestion};
use wsp_words::exit::HostError;

pub use crate::serve_broker::BrokerDoor;
pub use served::DynRuntime;
pub use socket::{FrameSink, Road, Socket};
pub use wsp_core::forwards::ForwardsSource;

/// How long a stopping host waits for a client to answer its close frame before the socket is cut. serve.ts:327-331.
pub const STOP_GRACE_MS: u64 = 1_000;
/// How long a ticket stands when the host names no other. serve.ts:388.
pub const TICKET_TTL_MS: u64 = 300_000;
/// More secret values than any real frame carries; past it the refusal's sentence is not scanned. serve.ts:350-352.
pub const SCANNED_SECRETS: usize = 256;
/// What `serve_runtime` refuses an empty host token with. serve.ts:385.
pub const NO_AUTH_TOKEN: &str = "serveRuntime refuses to start without an auth token";
/// What a socket that sent an unparseable frame is answered. serve.ts:639.
pub const INVALID_JSON: &str = "invalid json";
/// The sentences a door the host did not wire refuses its ops with. serve.ts:335, 378-384.
pub const NO_BUNDLER: &str = "this runtime cannot read folders on this computer";
pub const NO_LANDER: &str = "this runtime cannot write folders on this computer";
pub const NO_FOLDERS: &str = "this runtime cannot browse the folders on this computer";
pub const NO_TERMINAL_CONFIG: &str = "this runtime cannot read the terminal config on this computer";
pub const NO_EDITOR: &str = "this runtime cannot open an editor on this computer";
pub const NO_SSH: &str = "this runtime cannot carry an editor's ssh from this computer";
pub const NO_RELEASE: &str = "this runtime does not read the newest release";
pub const NO_INIT: &str = "this runtime has no cloud setup; the host that serves the app wires one";

/// The release a host runs and how it comes back, named on every answer that lets a socket in. serve.ts:141.
/// Written `version` then `road`, each only where given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Released {
    pub version: Option<String>,
    pub road: Option<String>,
}

/// What an upgrade said before it became a socket: the path with its query, the headers in arrival order, and the
/// peer. `origin_allowed`, `own_road` and `peer_of` read it (serve.ts:152-163).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeHead {
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    pub peer: Option<SocketAddr>,
}

/// A reading of an upgrade the host makes. serve.ts:155, 160, 163.
pub type UpgradeRule = Rc<dyn Fn(&UpgradeHead) -> bool>;
/// The milliseconds since the epoch, injectable for the ticket and pairing tests. serve.ts:182.
pub type NowFn = Rc<dyn Fn() -> i64>;
/// One line per refused frame, or one line of the host's own (serve.ts:217-219).
pub type LogFn = Rc<dyn Fn(&str)>;
/// Told of every op that answered with an error, after the answer is sent; it must not panic. serve.ts:216-217.
pub type FailedFn = Rc<dyn Fn(&str, &HostError)>;
/// The hosts the person's ssh knows on this computer, less the rows given. serve.ts:208-210.
pub type SshHostsFn = Rc<dyn Fn(Vec<PlaceView>) -> futures::future::LocalBoxFuture<'static, Result<Vec<SshHostSuggestion>, HostError>>>;
/// How a folder on this computer is read for project.plan and project.import. serve.ts:186-187.
pub type BundlerFn = Rc<dyn Fn(&str) -> Rc<dyn ProjectBundler>>;
/// A host source's listener's detach.
pub type Unlisten = Box<dyn FnOnce()>;

/// How the runtime asks the host who this wsp is signed in to. serve.ts:249-253.
#[async_trait(?Send)]
pub trait AccountDoor {
    async fn read(&self) -> Result<AccountView, HostError>;
}

/// The account's signer and its computers, for the one door a device with no code comes in by. serve.ts:255-267.
#[async_trait(?Send)]
pub trait AdmittedDevices {
    /// Nothing when this host is on no account, which is what refuses device.auth.
    fn signer(&self) -> Option<Signer>;
    /// Absent is unknown and never empty: a host that heard no listing admits nobody new.
    fn list(&self) -> Option<Vec<AccountDevice>>;
    /// One more beat, bounded by the host.
    async fn refresh(&self) -> Result<(), HostError>;
}

/// The key the computer that put this host on the account holds. serve.ts:260.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signer {
    pub fingerprint: String,
    pub public_key: String,
}

/// Where the door computers you own dial is, as the host opens it. serve.ts:269-277.
#[async_trait(?Send)]
pub trait PlaceDoorControl {
    /// The view less its host key (the place door's to say), with the door's loopback port where it has one.
    async fn open(&self) -> Result<OpenedDoor, HostError>;
}

/// `Omit<PlaceDoorView, "hostKey"> & { backPort?: number }`. serve.ts:276. `view.host_key` is ignored here; the
/// runtime puts the place door's on what it serves.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenedDoor {
    pub view: PlaceDoorView,
    pub back_port: Option<u16>,
}

/// The doctor's computer road as the host runs it. serve.ts:279-286.
#[async_trait(?Send)]
pub trait PlaceDoctor {
    async fn run(&self, place_id: &str, doctor_id: &str, project: Option<&str>) -> Result<i32, HostError>;
    /// A host source: its lines carry no sequence and are never replayed.
    fn on(&self, f: Rc<dyn Fn(&DoctorLineEvent)>) -> Unlisten;
}

/// The host's reading of the newest release. serve.ts:288-294.
#[async_trait(?Send)]
pub trait ReleaseDoor {
    fn get(&self) -> ReleaseView;
    async fn check(&self, force: bool) -> Result<ReleaseView, HostError>;
    fn on(&self, f: Rc<dyn Fn(&ReleaseChangedEvent)>) -> Unlisten;
}

/// How the host restarts itself. serve.ts:296-301.
#[async_trait(?Send)]
pub trait RestartDoor {
    /// Why the road this host came up on would not bring it back; none where it would.
    fn refusal(&self) -> Option<String>;
    /// Called once the reply is sent.
    async fn restart(&self) -> Result<(), HostError>;
}

/// Who a token names. serve.ts:303-305.
#[derive(Clone, Debug, PartialEq)]
pub enum Authed {
    Host,
    Device(HeldDevice),
}

/// serve.ts:139-220. Defaults are spec/serve_wire.md, W1.
#[derive(Clone, Default)]
pub struct ServeOptions {
    pub released: Option<Released>,
    /// A port of the runtime's own; 0 picks one. None: the runtime answers only the upgrades the host hands it.
    pub port: Option<u16>,
    pub auth_token: String,
    pub host: Option<String>,
    pub origin_allowed: Option<UpgradeRule>,
    pub own_road: Option<UpgradeRule>,
    pub peer_of: Option<Rc<dyn Fn(&UpgradeHead) -> String>>,
    pub door: Option<Rc<dyn PlaceDoorControl>>,
    pub devices: Option<Rc<dyn DeviceDoor>>,
    pub account: Option<Rc<dyn AccountDoor>>,
    pub admitted: Option<Rc<dyn AdmittedDevices>>,
    pub ticket_ttl_ms: Option<u64>,
    pub pair_ttl_ms: Option<u64>,
    pub auth_deadline_ms: Option<u64>,
    pub now: Option<NowFn>,
    pub forwards: Option<Rc<dyn ForwardsSource>>,
    pub broker: Option<Rc<dyn BrokerDoor>>,
    pub projects: Option<BundlerFn>,
    pub landing: Option<Rc<dyn ProjectLander>>,
    pub folders: Option<Rc<dyn HostFolders>>,
    pub terminal_config: Option<Rc<dyn HostTerminalConfig>>,
    pub editor: Option<Rc<dyn HostEditor>>,
    pub ssh: Option<Rc<dyn HostSsh>>,
    pub init: Option<Rc<dyn InitDoor>>,
    pub recipes: Option<Rc<dyn RecipeShelf>>,
    pub doctor: Option<Rc<dyn PlaceDoctor>>,
    pub ssh_hosts: Option<SshHostsFn>,
    pub release: Option<Rc<dyn ReleaseDoor>>,
    pub restart: Option<Rc<dyn RestartDoor>>,
    pub failed: Option<FailedFn>,
    pub log: Option<LogFn>,
}

/// An upgraded connection's bytes, as the host's HTTP server hands it over.
pub trait UpgradeIo: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin> UpgradeIo for T {}

/// What `serve_runtime` answers. serve.ts:307-317.
#[async_trait(?Send)]
pub trait RuntimeServer {
    /// The runtime's own port where it was given one, else `opts.port` or 0.
    fn port(&self) -> u16;
    /// Answers one upgrade any of the host's servers took (the TS's `attach`): refuses a wrong origin 403 and a path
    /// other than `WS_PATH` 400, else opens the socket. Spec W5.
    async fn upgrade(&self, head: UpgradeHead, io: Box<dyn UpgradeIo>);
    /// Who the bearer token of an HTTP request names; nothing for none or "". Spec W7.
    async fn authorize(&self, token: Option<&str>) -> Option<Authed>;
    /// Takes one device's token away and cuts the sockets it held. Spec W8.
    async fn revoke_device(&self, id: &str) -> Result<bool, HostError>;
    /// Closes every socket with 4001, cuts the rest after the grace. Spec W9.
    async fn close(&self) -> Result<(), HostError>;
}

/// Serves the runtime: binds its own port where given, and answers the upgrades the host hands it. serve.ts:375.
pub async fn serve_runtime(rt: Rc<DynRuntime>, opts: ServeOptions) -> Result<Rc<dyn RuntimeServer>, HostError> {
    let bind = opts.port.map(|port| (opts.host.clone().unwrap_or_else(|| LOOPBACK.to_string()), port));
    let served = door::Served::new(rt, opts)?;
    let server: Rc<dyn RuntimeServer> = Rc::new(listener::Wire::start(served, bind).await?);
    Ok(server)
}

/// Every workspace a verb answers with goes out through the schema that drops the display stream. serve.ts:372.
pub fn handed(workspace: WorkspaceView) -> serde_json::Value {
    serde_json::to_value(&workspace)
        .and_then(serde_json::from_value::<WorkspaceOut>)
        .and_then(serde_json::to_value)
        .unwrap_or(serde_json::Value::Null)
}

/// A start's answer with its workspace handed. serve.ts:373. `StartResult.workspace` is already a `WorkspaceOut`, so
/// the answer is the start's own fields as they serialize.
pub fn started_out(made: StartResult) -> serde_json::Value {
    serde_json::to_value(made).unwrap_or(serde_json::Value::Null)
}
