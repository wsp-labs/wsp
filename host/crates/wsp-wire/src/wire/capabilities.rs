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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder_disk_gb: Option<JsNumber>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitSetup {
    pub keys: InitKeys,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_provider: Option<String>,
    pub home: String,
    pub agents: Vec<InitAgent>,
    pub pricing: Option<InitSetupPricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<SetupPlace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_refusal: Option<String>,
    pub job: Option<InitJob>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSizeOffer {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub name: String,
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCapacityImage {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_taken: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_taken_mb: Option<JsNumber>,
    pub disk_free_bytes: JsNumber,
    pub images: Vec<PlaceCapacityImage>,
    pub machines: PlaceCapacityMachines,
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspaceSize {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <JsNumber as Check>::describe(rest),
                "memMb" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for InitSetup {
    fn check(&self) -> Result<(), SchemaError> {
        self.keys.check().map_err(|e| e.under("keys"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.pricing.check().map_err(|e| e.under("pricing"))?;
        self.place.check().map_err(|e| e.under("place"))?;
        self.job.check().map_err(|e| e.under("job"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "keys" => <InitKeys as Check>::describe(rest),
                "keyProvider" => <Option<String> as Check>::describe(rest),
                "home" => <String as Check>::describe(rest),
                "agents" => <Vec<InitAgent> as Check>::describe(rest),
                "pricing" => <Option<InitSetupPricing> as Check>::describe(rest),
                "place" => <Option<SetupPlace> as Check>::describe(rest),
                "forksOn" => <Option<String> as Check>::describe(rest),
                "buildRefusal" => <Option<String> as Check>::describe(rest),
                "job" => <Option<InitJob> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `builderDiskGb`: number min 0 exclusive
impl Check for InitSetupPricing {
    fn check(&self) -> Result<(), SchemaError> {
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.builder_disk_gb {
            crate::types::check::num_above(v.0, 0.0).map_err(|e| e.under("builderDiskGb"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "size" => <WorkspaceSize as Check>::describe(rest),
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "builderDiskGb" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineSizeOffer {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <JsNumber as Check>::describe(rest),
                "memMb" => <JsNumber as Check>::describe(rest),
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for Capabilities {
    fn check(&self) -> Result<(), SchemaError> {
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.sizes.check().map_err(|e| e.under("sizes"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "liveCloneForks" => <bool as Check>::describe(rest),
                "pauseMode" => <Option<PauseMode> as Check>::describe(rest),
                "replacesMachine" => <bool as Check>::describe(rest),
                "previewUrls" => <bool as Check>::describe(rest),
                "signedUrls" => <bool as Check>::describe(rest),
                "callbackRelay" => <bool as Check>::describe(rest),
                "diskSnapshots" => <bool as Check>::describe(rest),
                "images" => <bool as Check>::describe(rest),
                "snapshotsAnyLife" => <bool as Check>::describe(rest),
                "snapshotListing" => <bool as Check>::describe(rest),
                "templates" => <bool as Check>::describe(rest),
                "sizes" => <Vec<MachineSizeOffer> as Check>::describe(rest),
                "kept" => <bool as Check>::describe(rest),
                "copies" => <bool as Check>::describe(rest),
                "ownNetwork" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspaceLanding {
    fn check(&self) -> Result<(), SchemaError> {
        self.capabilities.check().map_err(|e| e.under("capabilities"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <Option<String> as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "capabilities" => <Capabilities as Check>::describe(rest),
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacity {
    fn check(&self) -> Result<(), SchemaError> {
        self.images.check().map_err(|e| e.under("images"))?;
        self.machines.check().map_err(|e| e.under("machines"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cores" => <JsNumber as Check>::describe(rest),
                "memMb" => <JsNumber as Check>::describe(rest),
                "memRoomMb" => <JsNumber as Check>::describe(rest),
                "machineMemMb" => <JsNumber as Check>::describe(rest),
                "cpuTaken" => <Option<JsNumber> as Check>::describe(rest),
                "memTakenMb" => <Option<JsNumber> as Check>::describe(rest),
                "diskFreeBytes" => <JsNumber as Check>::describe(rest),
                "images" => <Vec<PlaceCapacityImage> as Check>::describe(rest),
                "machines" => <PlaceCapacityMachines as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacityImage {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                "sizeBytes" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceCapacityMachines {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "running" => <JsNumber as Check>::describe(rest),
                "paused" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
    capabilities.sizes.is_empty()
}
// capabilities.ts:124
// /** Whether a place can build a copy of the image at all: it has to fork a builder and then copy that builder's disk
//  * into something a fork can stand on. A computer somebody joined does neither, and nor does a host with no provider
//  * key. The one place that reading is made, so the refusal and any road that offers the build read one rule. */
// export function buildsImages(capabilities: Pick<Capabilities, "diskSnapshots"> & { sizes: readonly MachineSizeOffer[] }): boolean {
//   return !forksNoMachines(capabilities) && capabilities.diskSnapshots;
// }
pub fn builds_images(capabilities: &Capabilities) -> bool {
    !forks_no_machines(capabilities) && capabilities.disk_snapshots
}
// capabilities.ts:131
// /** Whether a request asks for a size at all: a create naming none takes the golden's own size, and so never reads
//  * the list of sizes the provider offers. */
// export function namesSize(asked: Partial<WorkspaceSize> | undefined): boolean {
//   return asked?.cpu !== undefined || asked?.memMb !== undefined;
// }
pub fn names_size(cpu: Option<f64>, mem_mb: Option<f64>) -> bool {
    cpu.is_some() || mem_mb.is_some()
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
    let by_memory = (capacity.mem_room_mb.0 / mem_mb).floor();
    let by_disk = match image_bytes {
        Some(bytes) if bytes != 0.0 => (capacity.disk_free_bytes.0 / bytes).floor(),
        _ => f64::INFINITY,
    };
    // Math.min and Math.max answer NaN for a NaN; f64::min and f64::max would drop it.
    let least = if by_memory.is_nan() || by_disk.is_nan() {
        f64::NAN
    } else if by_memory < by_disk {
        by_memory
    } else {
        by_disk
    };
    if least.is_nan() {
        f64::NAN
    } else if least > 0.0 {
        least
    } else {
        0.0
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PauseMode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["memory", "disk"]))
    }
}
