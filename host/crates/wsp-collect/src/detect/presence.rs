// SPDX-License-Identifier: AGPL-3.0-only
//! Whether a catalog entry is on this computer. Port of packages/collect/src/detect/presence.ts at a3b870a6e; the
//! spec is spec/detect_small.md.
use crate::detect::common::found;
use crate::host::Host;

/// presence.ts:5: `found()`'s paths and bytes, and whether the entry's command is on PATH.
#[derive(Debug, Clone, PartialEq)]
pub struct Presence {
    /// The config paths that exist, `~`-relative, in the order given.
    pub paths: Vec<String>,
    /// Their bytes added up.
    pub bytes: u64,
    pub bin: bool,
}

/// Which of the entry's config paths exist and whether its command is on PATH; `None` when neither says so.
/// Which of the entry's config paths exist and whether its command is on PATH; `None` when neither says so.
pub async fn presence_of(host: &Host, config_paths: &[String], bin: &str) -> Option<Presence> {
    let f = found(host, config_paths).await;
    let on_path = host.exec.which(bin).await;
    if f.paths.is_empty() && !on_path {
        return None;
    }
    Some(Presence { paths: f.paths, bytes: f.bytes, bin: on_path })
}
