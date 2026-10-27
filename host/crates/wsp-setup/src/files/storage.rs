// SPDX-License-Identifier: AGPL-3.0-only
//! `storage.ts`: snapshot storage in words, the line wsp prints at start and the doctor's split of the account listing.
use wsp_core::types::api::{AccountOrphans, OrphansDeleted};
use wsp_engine::machine::SnapshotStoragePricing;
use wsp_wire::views::snapshot_lineage::SnapshotStorage;

/// `describeOwners` (`storage.ts:15`).
pub fn describe_owners(_s: &SnapshotStorage) -> String {
    todo!()
}

/// `noProviderStorageLine` (`storage.ts:27`).
pub fn no_provider_storage_line(_state_path: Option<&str>) -> String {
    todo!()
}

/// `describeStorage` (`storage.ts:30`).
pub fn describe_storage(_s: &SnapshotStorage, _place_name: Option<&str>) -> String {
    todo!()
}

/// `describeOrphans` (`storage.ts:45`).
pub fn describe_orphans(_plan: &AccountOrphans) -> Vec<String> {
    todo!()
}

/// `describeOrphanOffer` (`storage.ts:58`).
pub fn describe_orphan_offer(_plan: &AccountOrphans, _yes: bool, _pricing: &SnapshotStoragePricing, _state_path: Option<&str>) -> String {
    todo!()
}

/// `describeDeleted` (`storage.ts:75`).
pub fn describe_deleted(_done: &OrphansDeleted) -> String {
    todo!()
}
