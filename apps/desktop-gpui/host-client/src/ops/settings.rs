// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::{protocol, Done};

ops! {
    "account.get" => AccountGetRequest => Account,
    "capabilities.get" => CapabilitiesGetRequest => CapabilitiesReply,
    "devices.list" => DevicesListRequest => Devices,
    "devices.revoke" => DevicesRevokeRequest => Revoked,
    "editor.list" => EditorListRequest => Editors,
    "golden.get" => GoldenGetRequest => Golden,
    "image.build" => ImageBuildRequest => ImageBuild,
    "image.get" => ImageGetRequest => Image,
    "snapshots.list" => SnapshotsListRequest => Lineage,
    "snapshots.rollback" => SnapshotsRollbackRequest => protocol::SnapshotRollbackResult,
    "snapshots.storage" => SnapshotsStorageRequest => Storage,
    "projectGoldens.list" => ProjectGoldensListRequest => ProjectGoldens,
    "preferences.get" => PreferencesGetRequest => PreferencesReply,
    "preferences.set" => PreferencesSetRequest => PreferencesSet,
    "recipes.list" => RecipesListRequest => Recipes,
    "recipes.options" => RecipesOptionsRequest => RecipeOptions,
    "recipes.remove" => RecipesRemoveRequest => Recipe,
    "recipes.save" => RecipesSaveRequest => Recipe,
    "release.check" => ReleaseCheckRequest => Release,
    "release.get" => ReleaseGetRequest => Release,
    "ssh.include" => SshIncludeRequest => SshInclude,
    "host.restart" => HostRestartRequest => Done,
    "places.list" => PlacesListRequest => Places,
    "places.set" => PlacesSetRequest => Place,
    "places.follow" => PlacesFollowRequest => Place,
    "places.update" => PlacesUpdateRequest => protocol::PlaceUpdateReply,
    "places.remove" => PlacesRemoveRequest => protocol::PlaceRemoved,
    "places.holds" => PlacesHoldsRequest => protocol::PlaceHolds,
    "places.dial" => PlacesDialRequest => protocol::PlaceDial,
    "places.skip" => PlacesSkipRequest => Place,
    "places.setupLog" => PlacesSetupLogRequest => SetupLog,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub account: protocol::AccountView,
}

#[derive(Debug, Deserialize)]
pub struct CapabilitiesReply {
    pub capabilities: protocol::Capabilities,
}

#[derive(Debug, Deserialize)]
pub struct Devices {
    pub devices: Vec<protocol::DeviceView>,
}

#[derive(Debug, Deserialize)]
pub struct Revoked {
    pub revoked: bool,
}

#[derive(Debug, Deserialize)]
pub struct Editors {
    pub editors: Vec<protocol::EditorChoice>,
}

/// None for a golden never prepared.
#[derive(Debug, Deserialize)]
pub struct Golden {
    pub manifest: Option<protocol::GoldenManifest>,
}

#[derive(Debug, Deserialize)]
pub struct ImageBuild {
    pub build: protocol::SealedImageBuilt,
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub view: protocol::SealedImageView,
}

#[derive(Debug, Deserialize)]
pub struct Lineage {
    pub lineage: protocol::SnapshotLineage,
}

/// None on a backend that lists no snapshots.
#[derive(Debug, Deserialize)]
pub struct Storage {
    pub storage: Option<protocol::SnapshotStorage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGoldens {
    pub project_goldens: Vec<protocol::ProjectGolden>,
}

#[derive(Debug, Deserialize)]
pub struct PreferencesReply {
    pub preferences: protocol::Preferences,
}

/// `notice` is what the set kept but could not finish.
#[derive(Debug, Deserialize)]
pub struct PreferencesSet {
    pub preferences: protocol::Preferences,
    pub notice: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Recipes {
    pub recipes: Vec<protocol::RecipeView>,
}

#[derive(Debug, Deserialize)]
pub struct Recipe {
    pub recipe: protocol::RecipeView,
}

/// The protocol gives RecipeOptions as a TypeScript interface with no schema to record, so it is read untyped.
#[derive(Debug, Deserialize)]
pub struct RecipeOptions {
    pub options: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub release: protocol::ReleaseView,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshInclude {
    pub ssh_include: bool,
}

#[derive(Debug, Deserialize)]
pub struct Places {
    pub places: Vec<protocol::PlaceView>,
    #[serde(default)]
    pub adds: Vec<protocol::PlaceAddJob>,
    #[serde(default)]
    pub pending: Vec<protocol::PendingComputer>,
}

#[derive(Debug, Deserialize)]
pub struct Place {
    pub place: protocol::PlaceView,
}

#[derive(Debug, Deserialize)]
pub struct SetupLog {
    pub lines: Vec<String>,
}
