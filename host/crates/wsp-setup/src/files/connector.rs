// SPDX-License-Identifier: AGPL-3.0-only
//! `connector.ts`: the tunnel connector, its pinned release fetched and verified into the state folder, and the child it runs.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_words::exit::HostError;

/// `PinnedBinary.archive` (`connector.ts:21`), the one archive a pinned release may come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinnedArchive {
    Tgz,
}

/// `PinnedBinary` (`connector.ts:16`), one release for one kind of computer.
#[derive(Clone, Debug, PartialEq)]
pub struct PinnedBinary {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub archive: Option<PinnedArchive>,
}

/// `CloudflaredAsset` (`connector.ts:26`), one row of `CloudflaredTable.assets`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloudflaredAsset {
    pub url: String,
    pub sha256: String,
    pub archive: Option<String>,
}

/// `CloudflaredTable` (`connector.ts:24`), the pinned releases read from the table file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloudflaredTable {
    pub version: String,
    pub assets: IndexMap<String, CloudflaredAsset>,
}

/// `CLOUDFLARED` (`connector.ts:30`), the table read from `assets/cloudflared.json`.
pub fn cloudflared() -> CloudflaredTable {
    todo!()
}

/// `ConnectorArgsOptions` (`connector.ts:34`), the inline `opts` of `connectorArgs`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectorArgsOptions {
    pub token: Option<String>,
    pub port: u16,
}

/// `connectorArgs` (`connector.ts:34`).
pub fn connector_args(_opts: ConnectorArgsOptions) -> Vec<String> {
    todo!()
}

/// `pinnedCloudflared` (`connector.ts:39`).
pub fn pinned_cloudflared(_platform: &str, _arch: &str) -> Result<PinnedBinary, HostError> {
    todo!()
}

/// `FetchBytes` (`connector.ts:48`), the whole body of one URL, refused on a failed answer.
pub type FetchBytes = Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Vec<u8>, HostError>>>;

/// `httpBytes` (`connector.ts:51`).
pub async fn http_bytes(_url: &str) -> Result<Vec<u8>, HostError> {
    todo!()
}

/// `fetchPinned` (`connector.ts:61`).
pub async fn fetch_pinned(_dir: &str, _pin: &PinnedBinary, _fetch_bytes: FetchBytes) -> Result<String, HostError> {
    todo!()
}

/// `CloudflaredDeps` (`connector.ts:92`).
#[derive(Clone, Default)]
pub struct CloudflaredDeps {
    pub platform: Option<String>,
    pub arch: Option<String>,
    pub fetch_bytes: Option<FetchBytes>,
}

/// `ensureCloudflared` (`connector.ts:99`).
pub async fn ensure_cloudflared(_dir: &str, _deps: CloudflaredDeps) -> Result<String, HostError> {
    todo!()
}

/// `quickHostname` (`connector.ts:106`).
pub fn quick_hostname(_line: &str) -> Option<String> {
    todo!()
}

/// `ConnectorOptions` (`connector.ts:110`).
pub struct ConnectorOptions {
    pub bin: String,
    pub state_dir: String,
    pub port: u16,
    pub token: Option<String>,
    pub log: Rc<dyn Fn(&str)>,
    pub restart_ms: Option<u64>,
    pub keep_running: Option<Rc<dyn Fn() -> bool>>,
    pub on_hostname: Option<Rc<dyn Fn(&str)>>,
}

/// `Connector` (`connector.ts:129`), the connector a start runs and keeps running.
#[async_trait::async_trait(?Send)]
pub trait Connector {
    /// `Connector.pid` (`connector.ts:131`).
    fn pid(&self) -> Option<u32>;
    /// `Connector.hostname` (`connector.ts:133`).
    async fn hostname(&self) -> Option<String>;
    /// `Connector.stop` (`connector.ts:134`).
    async fn stop(&self);
}

/// `startConnector` (`connector.ts:141`).
pub fn start_connector(_opts: ConnectorOptions) -> Rc<dyn Connector> {
    todo!()
}

/// `connectorRunning` (`connector.ts:256`).
pub fn connector_running(_state_dir: &str) -> bool {
    todo!()
}

/// `stopRecordedConnector` (`connector.ts:266`).
pub async fn stop_recorded_connector(_state_dir: &str, _wait_ms: u64) {
    todo!()
}
