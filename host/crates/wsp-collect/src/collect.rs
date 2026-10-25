// SPDX-License-Identifier: AGPL-3.0-only
//! Every rung's detector in ladder order, validated against the manifest schema. Port of
//! packages/collect/src/collect.ts at a3b870a6e; the spec is spec/manifest.md.
use wsp_words::exit::HostError;

use crate::host::Host;
use crate::manifest::{Manifest, ManifestEntry, Rung};

/// `DETECTORS[rung](host, entries)` (collect.ts:15-26): identity, shell (detect_shell, then shell_sources over its rows,
/// then the terminal font rows), toolchains, tools, agents (detect_agents then detect_mcp) and logins. `entries` is
/// every row the earlier rungs found.
// collect.ts:15
// export const DETECTORS: Record<Rung, Detector> = {
//   identity: detectIdentity,
//   shell: async host => {
//     const rows = await detectShell(host);
//     await shellSources(host, rows);
//     return [...rows, ...(await detectTerminalFont(host))];
//   },
//   toolchains: detectToolchains,
//   tools: detectTools,
//   agents: async host => [...(await detectAgents(host)), ...(await detectMcp(host))],
//   logins: detectLogins,
// };
/// A detector's `Err` is returned at once; `collect` stops there with it and calls no later rung.
pub async fn run_detector(rung: Rung, host: &Host, entries: &[ManifestEntry]) -> Result<Vec<ManifestEntry>, HostError> {
    todo!()
}

/// collect.ts:28.
#[derive(Default)]
pub struct CollectOptions<'a> {
    /// Called after each rung's detector with its row count.
    pub on_rung: Option<&'a dyn Fn(Rung, usize)>,
}

/// Runs every rung's detector in ladder order and parses the rows with `parse_manifest`.
// collect.ts:33
// /** Runs every rung's detector in ladder order and validates the result against the schema. */
// export async function collect(host: Host, opts: CollectOptions = {}): Promise<Manifest> {
//   const entries: ManifestEntry[] = [];
//   for (const rung of RUNGS) {
//     const rows = await DETECTORS[rung](host, entries);
//     opts.onRung?.(rung, rows.length);
//     entries.push(...rows);
//   }
//   return parseManifest({ entries });
// }
pub async fn collect(host: &Host, opts: CollectOptions<'_>) -> Result<Manifest, HostError> {
    todo!()
}
