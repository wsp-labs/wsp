// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, refused, shared, Throwaway};

const NO_PLACE: &str = "settings-no-such-place";

#[tokio::test]
async fn about_and_account() {
    let client = shared().host.client().await;
    let account = answered("account.get", client.call(ask::<AccountGetRequest>(json!({}))).await);
    assert!(!account.account.signed_in);
    answered("capabilities.get", client.call(ask::<CapabilitiesGetRequest>(json!({}))).await);
    answered("devices.list", client.call(ask::<DevicesListRequest>(json!({}))).await);
    let revoked =
        answered("devices.revoke", client.call(ask::<DevicesRevokeRequest>(json!({ "deviceId": "settings-no-such-device" }))).await);
    assert!(!revoked.revoked);
    answered("editor.list", client.call(ask::<EditorListRequest>(json!({}))).await);
    answered("ssh.include", client.call(ask::<SshIncludeRequest>(json!({}))).await);
    answered("release.get", client.call(ask::<ReleaseGetRequest>(json!({}))).await);
}

#[tokio::test]
async fn preferences() {
    let client = shared().host.client().await;
    answered("preferences.get", client.call(ask::<PreferencesGetRequest>(json!({}))).await);
    let set =
        answered("preferences.set", client.call(ask::<PreferencesSetRequest>(json!({ "patch": { "cloudCardDismissed": true } }))).await);
    assert_eq!(set.preferences.cloud_card_dismissed, Some(true));
}

/// A patch moves only what it names: a field the protocol defaults is not sent unless asked for.
#[tokio::test]
async fn a_patch_leaves_what_it_does_not_name() {
    let host = Throwaway::start();
    let client = host.client().await;
    let off = json!({ "patch": { "productUsage": false, "lightTheme": "settings-theme" } });
    answered("preferences.set", client.call(ask::<PreferencesSetRequest>(off)).await);
    let set =
        answered("preferences.set", client.call(ask::<PreferencesSetRequest>(json!({ "patch": { "cloudCardDismissed": true } }))).await);
    assert!(!set.preferences.product_usage, "{:?}", set.preferences);
    assert_eq!(set.preferences.light_theme, "settings-theme");
}

#[tokio::test]
async fn image_and_snapshots() {
    let client = shared().host.client().await;
    answered("golden.get", client.call(ask::<GoldenGetRequest>(json!({ "name": "default" }))).await);
    let lineage = answered("snapshots.list", client.call(ask::<SnapshotsListRequest>(json!({}))).await);
    assert_eq!(lineage.lineage.name, "default");
    answered("snapshots.storage", client.call(ask::<SnapshotsStorageRequest>(json!({}))).await);
    refused(
        "snapshots.rollback",
        client.call(ask::<SnapshotsRollbackRequest>(json!({ "version": 999 }))).await,
        "no golden named \"default\"",
    );
    answered("projectGoldens.list", client.call(ask::<ProjectGoldensListRequest>(json!({}))).await);
    answered("image.get", client.call(ask::<ImageGetRequest>(json!({}))).await);
    refused(
        "image.build",
        client.call(ask::<ImageBuildRequest>(json!({ "place": NO_PLACE }))).await,
        &format!("no place named {NO_PLACE}"),
    );
}

#[tokio::test]
async fn recipes_and_computers() {
    let client = shared().host.client().await;
    answered("recipes.options", client.call(ask::<RecipesOptionsRequest>(json!({}))).await);
    let saved = answered("recipes.save", client.call(ask::<RecipesSaveRequest>(json!({ "name": "settings-recipe", "file": {} }))).await);
    assert_eq!(saved.recipe.name.to_string(), "settings-recipe");
    let recipes = answered("recipes.list", client.call(ask::<RecipesListRequest>(json!({}))).await);
    assert!(recipes.recipes.iter().any(|r| r.slug == saved.recipe.slug));

    let places = answered("places.list", client.call(ask::<PlacesListRequest>(json!({}))).await);
    assert!(places.places.iter().any(|p| p.id == "here"));
    answered("places.set", client.call(ask::<PlacesSetRequest>(json!({ "placeId": "here", "reset": ["max-depth"] }))).await);
    let unknown = format!("no place named {NO_PLACE}");
    refused(
        "places.follow",
        client.call(ask::<PlacesFollowRequest>(json!({ "placeId": NO_PLACE, "recipe": "settings-recipe" }))).await,
        &unknown,
    );
    refused("places.update", client.call(ask::<PlacesUpdateRequest>(json!({ "placeId": NO_PLACE }))).await, &unknown);
    let holds = answered("places.holds", client.call(ask::<PlacesHoldsRequest>(json!({ "placeId": NO_PLACE }))).await);
    assert!(holds.forks.is_empty() && holds.projects.is_empty());
    refused("places.dial", client.call(ask::<PlacesDialRequest>(json!({ "placeId": NO_PLACE }))).await, &unknown);
    refused("places.skip", client.call(ask::<PlacesSkipRequest>(json!({ "placeId": NO_PLACE, "row": "settings-row" }))).await, &unknown);
    refused("places.setupLog", client.call(ask::<PlacesSetupLogRequest>(json!({ "placeId": NO_PLACE }))).await, &unknown);
    let removed = answered("places.remove", client.call(ask::<PlacesRemoveRequest>(json!({ "placeId": NO_PLACE }))).await);
    assert!(!removed.removed);

    let gone = answered("recipes.remove", client.call(ask::<RecipesRemoveRequest>(json!({ "name": "settings-recipe" }))).await);
    assert_eq!(gone.recipe.slug, saved.recipe.slug);
}

#[tokio::test]
async fn release_check_and_restart() {
    let host = Throwaway::start();
    std::fs::write(host.state().with_file_name(".env"), "WSP_UPDATE_CHECK=0\n").unwrap();
    let client = host.client().await;
    let release = answered("release.check", client.call(ask::<ReleaseCheckRequest>(json!({ "force": true }))).await);
    assert_eq!(release.release.state, ReleaseViewState::Off);
    refused(
        "host.restart",
        client.call(ask::<HostRestartRequest>(json!({}))).await,
        "Ctrl-C the terminal running wsp up and run it again.",
    );
}
