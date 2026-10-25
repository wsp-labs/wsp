// SPDX-License-Identifier: AGPL-3.0-only
//! Whether a catalog entry is on this computer. Port of packages/collect/src/detect/presence.ts at a3b870a6e; the
//! spec is spec/detect_small.md.
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
// detect/presence.ts:10
// /** Whether a catalog entry is on this computer: which of its config paths exist and whether its command is on
//  * PATH; nothing when neither says so. The one rule every presence check reads. */
// export async function presenceOf(host: Host, e: { configPaths: readonly string[]; bin: string }): Promise<Presence | undefined> {
//   const f = await found(host, e.configPaths);
//   const bin = await host.exec.which(e.bin);
//   return f.paths.length === 0 && !bin ? undefined : { ...f, bin };
// }
pub async fn presence_of(host: &Host, config_paths: &[String], bin: &str) -> Option<Presence> {
    todo!()
}
