// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:867-878 and 1265-1320: the sha an updated version records, and the smoke, missing and left-behind lists
//! an update carries forward.
use indexmap::IndexSet;
use wsp_wire::views::golden_image::{GoldenLeftBehind, GoldenMissingTool};

use super::types::GoldenDelta;
use crate::golden_seam::GoldenImport;

/// golden.ts:868. An updated version's sha chains the version it came from with what the delta ran.
pub fn next_setup_sha(_previous: &str, _setup: &str, _imp: &GoldenImport) -> String {
    todo!("golden.ts:868")
}

/// golden.ts:873. Pins which installers ran: the harness line and every agent's.
pub fn setup_sha_of(_setup: &str, _imp: Option<&GoldenImport>) -> String {
    todo!("golden.ts:873")
}

/// golden.ts:1267. The agents' version checks as one command for the fork; none is `true`.
pub fn join_smoke(_checks: &[String]) -> String {
    todo!("golden.ts:1267")
}

/// golden.ts:1275. A tools row's version check where its tool is a manager a project's install runs.
pub fn manager_check_of(_id: &str) -> Option<String> {
    todo!("golden.ts:1275")
}

/// golden.ts:1279.
pub fn smoke_checks(_smoke: &str) -> Vec<String> {
    todo!("golden.ts:1279")
}

/// golden.ts:1285. The version checks of what is on the image after the delta: the previous smoke without the removed
/// agents', joined with the added agents'.
pub fn next_smoke(_previous: &str, _removed: &[String], _added: &str) -> String {
    todo!("golden.ts:1285")
}

/// golden.ts:1293. What a passed smoke proved, for the stage's end line.
pub fn smoke_tally(_smoke: &str) -> String {
    todo!("golden.ts:1293")
}

/// golden.ts:1303. The recipe rows a delta addresses: what it retires and what it plans again.
pub fn touched_by(_delta: &GoldenDelta) -> IndexSet<String> {
    todo!("golden.ts:1303")
}

/// golden.ts:1310. What an updated image is missing.
pub fn next_missing(_previous: &[GoldenMissingTool], _delta: &GoldenDelta, _fresh: &[GoldenMissingTool]) -> Vec<GoldenMissingTool> {
    todo!("golden.ts:1310")
}

/// golden.ts:1317. What an updated image is without.
pub fn next_left_behind(_previous: &[GoldenLeftBehind], _delta: &GoldenDelta, _fresh: &[GoldenLeftBehind]) -> Vec<GoldenLeftBehind> {
    todo!("golden.ts:1317")
}

/// golden.ts:1471. Moves the head to a version the manifest holds; refuses one it does not.
pub fn rollback(
    _manifest: &wsp_wire::views::golden_image::GoldenManifest,
    _version: u32,
) -> Result<wsp_wire::views::golden_image::GoldenManifest, crate::errors::EngineError> {
    todo!("golden.ts:1471")
}
