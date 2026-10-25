// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/land-bytes.ts. Spec: spec/machine_reads.md.
//!
//! One road for putting a file's bytes on a machine, whichever backend holds it: the backend's own road where it has
//! one, else the provider's signed upload URL.
use std::sync::Arc;

use wsp_wire::wire::capabilities::Capabilities;

use crate::errors::{EngineError, Result};
use crate::fetch::{platform_fetch, Fetch, FetchRequest};
use crate::machine::{BytesLanded, Machine};

/// land-bytes.ts:9.
#[derive(Clone, Default)]
pub struct LandBytesOptions {
    /// The upload's transport for the signed URL road; tests hand in their own.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// How long the backend's own road may take; its own pace when `None`. The signed URL road never reads it.
    pub timeout_ms: Option<u64>,
}

/// land-bytes.ts:18. Whether the machine carries a road of the backend's own for bytes: `machine.offers().put_bytes`.
pub fn has_byte_road(machine: &dyn Machine) -> bool {
    machine.offers().put_bytes
}

/// land-bytes.ts:24. The provider mints a signed URL, or the backend carries its own road.
pub fn lands_bytes(capabilities: &Capabilities, machine: &dyn Machine) -> bool {
    capabilities.signed_urls || has_byte_road(machine)
}

/// land-bytes.ts:30. The backend's own road first: `put_bytes(path, bytes, timeout_ms)` and its answer as it came.
/// Else `upload_url(path)` and one `PUT` of the bytes to it, no headers, answering `None`; a status outside 200..300 is
/// an `Other` with `<path> did not land on <machine id>: HTTP <status>`. A failed fetch is the failure as it was.
pub async fn land_bytes(machine: &dyn Machine, path: &str, bytes: &[u8], opts: LandBytesOptions) -> Result<Option<BytesLanded>> {
    if has_byte_road(machine) {
        return machine.put_bytes(path, bytes, opts.timeout_ms).await;
    }
    let url = machine.upload_url(path).await?;
    let fetch = opts.fetch.clone().unwrap_or_else(platform_fetch);
    let req = FetchRequest { method: "PUT".into(), url, headers: Vec::new(), body: Some(bytes.to_vec()), abort: None };
    let put = fetch.fetch(req).await.map_err(EngineError::Road)?;
    if !put.ok() {
        return Err(EngineError::Other {
            message: format!("{path} did not land on {}: HTTP {}", machine.id(), put.status),
            code: None,
        });
    }
    Ok(None)
}
