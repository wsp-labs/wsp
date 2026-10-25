// SPDX-License-Identifier: AGPL-3.0-only
//! What a socket serves (spec/README.md, "The seams"): wsp-core's `Runtime`, whose three area doors are the traits of
//! the crates that hold them. wsp-core sits below wsp-machines, wsp-slates and wsp-threads and cannot name them, so
//! `Runtime` says which they are through its associated types.
use wsp_core::types::api::Runtime;
use wsp_machines::places::types::PlaceDoor;
use wsp_slates::slates::Slates;
use wsp_threads::activity::ActivityDoor;

/// A frame a road pushes or answers with: one JSON object, written as given.
pub type Payload = serde_json::Map<String, serde_json::Value>;

/// The runtime a socket serves: `rt.places` is wsp-machines' `PlaceDoor`, `rt.slates` wsp-slates' `Slates` and
/// `rt.activity` wsp-threads' `ActivityDoor`. Implemented by `AssembledRuntime` (spec/runtime_of.md) and by the
/// tests' fake.
pub type DynRuntime = dyn Runtime<Places = dyn PlaceDoor, Slates = dyn Slates, Activity = dyn ActivityDoor>;
