// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/capabilities.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::init_job::{InitAgent, InitJob, InitKeys, SetupPlace};
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::workspace_kind::WorkspaceKind;
use wsp_frames::{False, True};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSize {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitSetupPricing {
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub builder_disk_gb: Option<JsNumber>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitSetup {
    pub keys: InitKeys,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub key_provider: Option<String>,
    pub home: String,
    pub agents: Vec<InitAgent>,
    pub pricing: Option<InitSetupPricing>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<SetupPlace>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forks_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub build_refusal: Option<String>,
    pub job: Option<InitJob>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSizeOffer {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PauseMode {
    #[serde(rename = "memory")]
    Memory,
    #[serde(rename = "disk")]
    Disk,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub live_clone_forks: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    pub replaces_machine: bool,
    pub preview_urls: bool,
    pub signed_urls: bool,
    pub callback_relay: bool,
    pub disk_snapshots: bool,
    pub images: bool,
    pub snapshots_any_life: bool,
    pub snapshot_listing: bool,
    pub templates: bool,
    pub sizes: Vec<MachineSizeOffer>,
    pub kept: bool,
    pub copies: bool,
    pub own_network: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLanding {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    pub name: String,
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCapacityImage {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    pub size_bytes: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCapacityMachines {
    pub running: JsNumber,
    pub paused: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCapacity {
    pub cores: JsNumber,
    pub mem_mb: JsNumber,
    pub mem_room_mb: JsNumber,
    pub machine_mem_mb: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu_taken: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mem_taken_mb: Option<JsNumber>,
    pub disk_free_bytes: JsNumber,
    pub images: Vec<PlaceCapacityImage>,
    pub machines: PlaceCapacityMachines,
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspaceSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for InitSetup {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("keys") { None => out.push(crate::types::check::missing::<Self>("keys")), Some(v) => { match crate::types::check::read::<InitKeys>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keys"))), } } }
        if let Some(v) = obj.get("keyProvider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keyProvider"))), } }
        match obj.get("home") { None => out.push(crate::types::check::missing::<Self>("home")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<InitAgent>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        if let Some(v) = obj.get("pricing") { match crate::types::check::read_present::<InitSetupPricing>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pricing"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<SetupPlace>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("forksOn") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forksOn"))), } }
        if let Some(v) = obj.get("buildRefusal") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("buildRefusal"))), } }
        if let Some(v) = obj.get("job") { match crate::types::check::read_present::<InitJob>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("job"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `builderDiskGb`: number min 0 exclusive
impl Check for InitSetupPricing {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        match obj.get("rateUsdPerHour") { None => out.push(crate::types::check::missing::<Self>("rateUsdPerHour")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } } }
        if let Some(v) = obj.get("builderDiskGb") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("builderDiskGb"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineSizeOffer {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        match obj.get("rateUsdPerHour") { None => out.push(crate::types::check::missing::<Self>("rateUsdPerHour")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } } }
        if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for Capabilities {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("liveCloneForks") { None => out.push(crate::types::check::missing::<Self>("liveCloneForks")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("liveCloneForks"))), } } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        match obj.get("replacesMachine") { None => out.push(crate::types::check::missing::<Self>("replacesMachine")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replacesMachine"))), } } }
        match obj.get("previewUrls") { None => out.push(crate::types::check::missing::<Self>("previewUrls")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("previewUrls"))), } } }
        match obj.get("signedUrls") { None => out.push(crate::types::check::missing::<Self>("signedUrls")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signedUrls"))), } } }
        match obj.get("callbackRelay") { None => out.push(crate::types::check::missing::<Self>("callbackRelay")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("callbackRelay"))), } } }
        match obj.get("diskSnapshots") { None => out.push(crate::types::check::missing::<Self>("diskSnapshots")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskSnapshots"))), } } }
        match obj.get("images") { None => out.push(crate::types::check::missing::<Self>("images")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("images"))), } } }
        match obj.get("snapshotsAnyLife") { None => out.push(crate::types::check::missing::<Self>("snapshotsAnyLife")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotsAnyLife"))), } } }
        match obj.get("snapshotListing") { None => out.push(crate::types::check::missing::<Self>("snapshotListing")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotListing"))), } } }
        match obj.get("templates") { None => out.push(crate::types::check::missing::<Self>("templates")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("templates"))), } } }
        match obj.get("sizes") { None => out.push(crate::types::check::missing::<Self>("sizes")), Some(v) => { match crate::types::check::read::<Vec<MachineSizeOffer>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizes"))), } } }
        match obj.get("kept") { None => out.push(crate::types::check::missing::<Self>("kept")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kept"))), } } }
        match obj.get("copies") { None => out.push(crate::types::check::missing::<Self>("copies")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copies"))), } } }
        match obj.get("ownNetwork") { None => out.push(crate::types::check::missing::<Self>("ownNetwork")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ownNetwork"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspaceLanding {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("capabilities") { None => out.push(crate::types::check::missing::<Self>("capabilities")), Some(v) => { match crate::types::check::read::<Capabilities>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("capabilities"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacity {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cores") { None => out.push(crate::types::check::missing::<Self>("cores")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cores"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        match obj.get("memRoomMb") { None => out.push(crate::types::check::missing::<Self>("memRoomMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memRoomMb"))), } } }
        match obj.get("machineMemMb") { None => out.push(crate::types::check::missing::<Self>("machineMemMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineMemMb"))), } } }
        if let Some(v) = obj.get("cpuTaken") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpuTaken"))), } }
        if let Some(v) = obj.get("memTakenMb") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memTakenMb"))), } }
        match obj.get("diskFreeBytes") { None => out.push(crate::types::check::missing::<Self>("diskFreeBytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskFreeBytes"))), } } }
        match obj.get("images") { None => out.push(crate::types::check::missing::<Self>("images")), Some(v) => { match crate::types::check::read::<Vec<PlaceCapacityImage>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("images"))), } } }
        match obj.get("machines") { None => out.push(crate::types::check::missing::<Self>("machines")), Some(v) => { match crate::types::check::read::<PlaceCapacityMachines>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacityImage {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
        match obj.get("sizeBytes") { None => out.push(crate::types::check::missing::<Self>("sizeBytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeBytes"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacityMachines {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("running") { None => out.push(crate::types::check::missing::<Self>("running")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } } }
        match obj.get("paused") { None => out.push(crate::types::check::missing::<Self>("paused")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paused"))), } } }
        Some(out)
    }

}

// capabilities.ts:117
// /** Whether this host forks no machine at all: the provider module a keyless host wires offers no size, so the roads
//  * that would fork one answer NO_PROVIDER_LINE instead of sending the person back to an init that seals nothing. The
//  * one place that reading is made, so nothing above the provider module asks whether there is a key. */
// export function forksNoMachines(capabilities: { sizes: readonly MachineSizeOffer[] }): boolean {
//   return capabilities.sizes.length === 0;
// }
pub fn forks_no_machines(capabilities: &Capabilities) -> bool {
    todo!()
}
// capabilities.ts:124
// /** Whether a place can build a copy of the image at all: it has to fork a builder and then copy that builder's disk
//  * into something a fork can stand on. A computer somebody joined does neither, and nor does a host with no provider
//  * key. The one place that reading is made, so the refusal and any road that offers the build read one rule. */
// export function buildsImages(capabilities: Pick<Capabilities, "diskSnapshots"> & { sizes: readonly MachineSizeOffer[] }): boolean {
//   return !forksNoMachines(capabilities) && capabilities.diskSnapshots;
// }
pub fn builds_images(capabilities: &Capabilities) -> bool {
    todo!()
}
// capabilities.ts:131
// /** Whether a request asks for a size at all: a create naming none takes the golden's own size, and so never reads
//  * the list of sizes the provider offers. */
// export function namesSize(asked: Partial<WorkspaceSize> | undefined): boolean {
//   return asked?.cpu !== undefined || asked?.memMb !== undefined;
// }
pub fn names_size(cpu: Option<f64>, mem_mb: Option<f64>) -> bool {
    todo!()
}
// capabilities.ts:159
// /** How many more forks a place takes: the memory rule, and the disk rule where an image is there to measure by.
//  * One function, read by the command line's table and the app's place row. */
// export function forkRoom(c: Pick<PlaceCapacity, "memRoomMb" | "diskFreeBytes">, memMb: number, imageBytes: number | undefined): number {
//   const byMemory = Math.floor(c.memRoomMb / memMb);
//   const byDisk = imageBytes === undefined || imageBytes === 0 ? Number.POSITIVE_INFINITY : Math.floor(c.diskFreeBytes / imageBytes);
//   return Math.max(0, Math.min(byMemory, byDisk));
// }
pub fn fork_room(capacity: &PlaceCapacity, mem_mb: f64, image_bytes: Option<f64>) -> f64 {
    todo!()
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PauseMode {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["memory", "disk"]))
    }
}
