// SPDX-License-Identifier: AGPL-3.0-only
//! Every rung's detector in ladder order, validated against the manifest schema. Port of
//! packages/collect/src/collect.ts at a3b870a6e; the spec is spec/manifest.md.
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::detect::agents::detect_agents;
use crate::detect::identity::detect_identity;
use crate::detect::logins::detect_logins;
use crate::detect::mcp::detect_mcp;
use crate::detect::shell::detect_shell;
use crate::detect::sources::shell_sources;
use crate::detect::terminal::detect_terminal_font;
use crate::detect::toolchains::detect_toolchains;
use crate::detect::tools::detect_tools;
use crate::host::Host;
use crate::manifest::{parse_manifest, Manifest, ManifestEntry, Rung, RUNGS};

/// `DETECTORS[rung](host, entries)` (collect.ts:15-26): identity, shell (detect_shell, then shell_sources over its rows,
/// then the terminal font rows), toolchains, tools, agents (detect_agents then detect_mcp) and logins. `entries` is
/// every row the earlier rungs found.
/// A detector's `Err` is returned at once; `collect` stops there with it and calls no later rung.
pub async fn run_detector(rung: Rung, host: &Host, _entries: &[ManifestEntry]) -> Result<Vec<ManifestEntry>, HostError> {
    Ok(match rung {
        Rung::Identity => detect_identity(host).await,
        Rung::Shell => {
            let mut rows = detect_shell(host).await;
            shell_sources(host, &mut rows).await;
            rows.extend(detect_terminal_font(host).await);
            rows
        }
        Rung::Toolchains => detect_toolchains(host).await,
        Rung::Tools => detect_tools(host).await?,
        Rung::Agents => {
            let mut rows = detect_agents(host).await?;
            rows.extend(detect_mcp(host, None).await);
            rows
        }
        Rung::Logins => detect_logins(host).await,
    })
}

/// collect.ts:28.
#[derive(Default)]
pub struct CollectOptions<'a> {
    /// Called after each rung's detector with its row count.
    pub on_rung: Option<&'a dyn Fn(Rung, usize)>,
}

/// Runs every rung's detector in ladder order and parses the rows with `parse_manifest`.
pub async fn collect(host: &Host, opts: CollectOptions<'_>) -> Result<Manifest, HostError> {
    let mut entries: Vec<ManifestEntry> = Vec::new();
    for rung in RUNGS {
        let rows = run_detector(rung, host, &entries).await?;
        if let Some(on_rung) = opts.on_rung {
            on_rung(rung, rows.len());
        }
        entries.extend(rows);
    }
    let data: Value = serde_json::to_value(Manifest { entries }).map_err(|e| HostError::new(e.to_string()))?;
    parse_manifest(&data)
}
