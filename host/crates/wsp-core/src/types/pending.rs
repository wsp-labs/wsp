// SPDX-License-Identifier: AGPL-3.0-only
//! The names the five types files import, in one place. A name with a Rust home is a `pub use` of it; the rest are
//! stand-ins for homes not written yet (the image roads' types in the engine, `github_cache`), each naming its TS
//! home, and each becomes a `pub use` when its home lands. Not a TS file of its own.

use std::sync::Arc;

use indexmap::IndexMap;
use wsp_engine::machine::Machine;
use wsp_wire::views::golden_image::{GoldenBaseTool, GoldenLeftBehind, GoldenMissingTool, GoldenRetired, MachineKind, RecipeDigest};
use wsp_wire::wire::capabilities::WorkspaceSize;

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
pub use crate::github_cache::GitHubCache;

/// TS engine golden-import.ts:817: `{ HOME: GUEST_HOME, USER: "root" }`. The engine has no Rust home for it yet.
pub fn guest_user_env() -> IndexMap<String, String> {
    let mut env = IndexMap::new();
    env.insert("HOME".into(), wsp_catalog::roads::GUEST_HOME.into());
    env.insert("USER".into(), "root".into());
    env
}

/// TS engine `BuildGoldenOptions`.
#[derive(Clone, Debug, Default)]
pub struct BuildGoldenOptions {}
/// TS engine golden.ts:405 `Builder`: a builder machine as the image roads hold it. Replaced whole, never changed in
/// place (`kept.builder = { ...kept.builder, ... }`).
#[derive(Clone)]
pub struct Builder {
    pub machine: Arc<dyn Machine>,
    pub kind: MachineKind,
    pub base_template: String,
    pub setup_sha: String,
    pub created_at: String,
    pub first_life: bool,
    pub size: WorkspaceSize,
    pub import: Option<ImportLedger>,
    pub parent_snapshot_id: Option<String>,
    pub base: Option<Vec<GoldenBaseTool>>,
    pub retired: Option<Vec<GoldenRetired>>,
}
/// TS engine `GoldenDelta`.
#[derive(Clone, Debug, Default)]
pub struct GoldenDelta {}
/// TS engine golden.ts:520 `ImportStage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportStage {
    #[serde(rename = "applying-setup")]
    ApplyingSetup,
    #[serde(rename = "uploading-files")]
    UploadingFiles,
    #[serde(rename = "installing-tools")]
    InstallingTools,
    #[serde(rename = "installing-harness")]
    InstallingHarness,
    #[serde(rename = "installing-mcp")]
    InstallingMcp,
}

/// TS engine golden.ts:522 `ImportLedger`: what of a recipe is on a builder, in the TS's key order.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLedger {
    pub recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeDigest>,
    pub applied: Vec<ImportStage>,
    /// The version checks of the agents that installed, joined; the seal runs this on the fork.
    pub smoke: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_tools: Option<Vec<GoldenMissingTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silenced: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_noise: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_behind: Option<Vec<GoldenLeftBehind>>,
}
/// TS engine `KillConfirm`.
#[derive(Clone, Debug, Default)]
pub struct KillConfirm {}
/// TS engine `Sighting`: what a provider read found of a machine.
#[derive(Clone, Debug, Default)]
pub struct Sighting {}

// Area types that rule 1.5 moves down into wsp-core because `types/api.ts` and `types/wiring.ts` name them. Each is
// a handle here; the area writer of its TS file declares its methods on it.

/// TS runtime places.ts `PlaceWiring` (wsp-machines).
pub trait PlaceWiring {}
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
    /// The row a server's sign-in was started from, which the list of sign-ins hands back (agents-read.ts:105).
    pub scope: Option<wsp_wire::types::agents_report::McpScope>,
    pub project: Option<String>,
}
/// TS agents-read.ts:148.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillAsk {
    pub name: String,
    pub project: Option<bool>,
}
