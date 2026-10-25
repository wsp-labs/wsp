// SPDX-License-Identifier: AGPL-3.0-only
//! The names the five types files import, in one place. A name with a Rust home is a `pub use` of it; the rest are
//! stand-ins for homes not written yet (the image roads' types in the engine, `github_cache`), each naming its TS
//! home, and each becomes a `pub use` when its home lands. Not a TS file of its own.

use indexmap::IndexMap;

pub use wsp_engine::copier::Copier;
pub use wsp_engine::golden_seam::GoldenImport;
pub use wsp_engine::vault::CacheRule;
pub use wsp_store::store::{BlobMark, Store};

pub use crate::clock::{real_clock, Clock};
pub use crate::daemon_channel::{DaemonChannel, DaemonChannelOptions};
pub use crate::devices::{DeviceDoor, IssuedCode, PairedDevice, ScopedRoad};
pub use crate::machine_exec::{MachineExecOptions, TurnWaiting};
pub use crate::reach::DaemonReach;
pub use crate::status::{StatusApi, StatusListOptions, StatusWatchOptions};

/// Where the reads off a git host keep their bodies and ETags. TS runtime github-cache.ts, wsp-core's `github_cache`.
pub trait GitHubCache {}

/// TS engine golden-import.ts:817: `{ HOME: GUEST_HOME, USER: "root" }`. The engine has no Rust home for it yet.
pub fn guest_user_env() -> IndexMap<String, String> {
    todo!()
}

/// TS engine `BuildGoldenOptions`.
#[derive(Clone, Debug, Default)]
pub struct BuildGoldenOptions {}
/// TS engine `Builder`: a builder machine as the image roads hold it.
pub trait Builder {}
/// TS engine `GoldenDelta`.
#[derive(Clone, Debug, Default)]
pub struct GoldenDelta {}
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
