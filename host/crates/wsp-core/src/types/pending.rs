// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins for the names the five types files import whose Rust home is not written yet, so the crate compiles
//! while wave 4's other slices are written beside it. Each names its TS home; once that home lands, its line here
//! becomes a `pub use` of it and nothing else changes. Not a TS file of its own.

use std::rc::Rc;

use indexmap::IndexMap;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::HostError;

pub use crate::clock::{real_clock, Clock};

pub use crate::daemon_channel::{DaemonChannel, DaemonChannelOptions};

/// A turn's stream options on a machine. TS runtime machine-exec.ts:47, wsp-core's `machine_exec` module.
#[derive(Clone, Debug, Default)]
pub struct MachineExecOptions {}
/// Whether the run is stopped on a question only a person can answer. TS machine-exec.ts:222.
pub type TurnWaiting = Rc<dyn Fn() -> bool>;

pub use crate::status::{StatusApi, StatusListOptions, StatusWatchOptions};

pub use crate::devices::{DeviceDoor, IssuedCode, PairedDevice, ScopedRoad};
pub use crate::reach::DaemonReach;

/// Where the reads off a git host keep their bodies and ETags. TS runtime github-cache.ts, wsp-core's `github_cache`.
pub trait GitHubCache {}

/// The host's store. TS runtime store.ts:9-20, wsp-store's row in chapter 01's table. `transcripts` is the store
/// writer's to add.
#[async_trait::async_trait(?Send)]
pub trait Store {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>, HostError>;
    async fn put(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<(), HostError>;
    async fn list(&self, collection: &str) -> Result<Vec<serde_json::Value>, HostError>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError>;
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError>;
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError>;
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError>;
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError>;
}
/// The size and write time of a blob, which an index names the transcript it was read off by. TS store.ts:26-29.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlobMark {
    pub bytes: JsNumber,
    pub at: JsNumber,
}

/// TS engine golden-import.ts:817: `{ HOME: GUEST_HOME, USER: "root" }`.
pub fn guest_user_env() -> IndexMap<String, String> {
    todo!()
}

/// TS engine `BuildGoldenOptions`.
#[derive(Clone, Debug, Default)]
pub struct BuildGoldenOptions {}
/// TS engine `Builder`: a builder machine as the image roads hold it.
pub trait Builder {}
/// TS engine `CacheRule`: what a bundle leaves behind.
#[derive(Clone, Debug, Default)]
pub struct CacheRule {}
/// TS engine `Copier`: the daemon binary's copy verb on this computer.
pub trait Copier {}
/// TS engine `GoldenDelta`.
#[derive(Clone, Debug, Default)]
pub struct GoldenDelta {}
/// TS engine `GoldenImport`.
#[derive(Clone, Debug, Default)]
pub struct GoldenImport {}
/// TS engine `ImportLedger`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportLedger {}
/// TS engine `KillConfirm`.
#[derive(Clone, Debug, Default)]
pub struct KillConfirm {}
/// TS engine `Sighting`: what a provider read found of a machine.
#[derive(Clone, Debug, Default)]
pub struct Sighting {}

// Area types that rule 1.5 moves down into wsp-core because `types/api.ts` and `types/wiring.ts` name them. Each is
// a handle here; the area writer of its TS file declares its methods on it.

/// TS runtime places.ts `PlaceDoor` (wsp-machines).
pub trait PlaceDoor {}
/// TS runtime places.ts `PlaceWiring` (wsp-machines).
pub trait PlaceWiring {}
/// TS runtime slates.ts `Slates` (wsp-slates).
pub trait Slates {}
/// TS runtime activity.ts `ActivityDoor` (wsp-threads).
pub trait ActivityDoor {}
/// TS runtime agents-read.ts `AgentsReader` (wsp-machines).
pub trait AgentsReader {}
/// TS agents-read.ts `AgentsActs`.
pub trait AgentsActs {}
/// TS agents-read.ts `SkillsActs`.
pub trait SkillsActs {}
/// TS agents-read.ts `ServersActs`.
pub trait ServersActs {}
/// TS agents-read.ts `PluginsActs`.
pub trait PluginsActs {}
/// TS agents-read.ts `ServerIcons`.
pub trait ServerIcons {}
/// TS agents-read.ts:129 `CallbackForwards`.
pub trait CallbackForwards {}
/// TS agents-read.ts:93.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignInAsk {
    pub agent: String,
    pub server: Option<String>,
    pub terminal: Option<bool>,
}
/// TS agents-read.ts:148.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillAsk {
    pub name: String,
    pub project: Option<bool>,
}
