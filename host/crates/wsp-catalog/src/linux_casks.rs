// SPDX-License-Identifier: AGPL-3.0-only
//! Commands with a Linux build of their own that the vendor road installs, each pinned with a sum per arch. Port of
//! packages/catalog/src/linux-casks.ts at a3b870a6e; the spec is spec/roads.md.
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// linux-casks.ts:387.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxCask {
    pub bin: String,
    pub from: String,
    pub detail: String,
    pub version: String,
    pub sha256: CaskSums,
    pub install: String,
    pub uninstall: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaskSums {
    pub x86_64: String,
    pub aarch64: String,
}

pub static GCLOUD: LazyLock<LinuxCask> = LazyLock::new(|| todo!());
pub static KUBECTL: LazyLock<LinuxCask> = LazyLock::new(|| todo!());
pub static CURSOR: LazyLock<LinuxCask> = LazyLock::new(|| todo!());

/// gcloud, kubectl, cursor-agent, in that order (linux-casks.ts:507).
pub fn linux_casks() -> Vec<&'static LinuxCask> {
    todo!()
}
