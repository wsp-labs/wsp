// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:867-878 and 1265-1320: the sha an updated version records, and the smoke, missing and left-behind lists
//! an update carries forward.
use std::collections::HashSet;
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use sha2::{Digest, Sha256};
use wsp_catalog::catalog::{catalog_agents, catalog_entry, catalog_id_of_row, smoke_of};
use wsp_catalog::seed::SEED_ROWS;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{GoldenLeftBehind, GoldenManifest, GoldenMissingTool};
use wsp_words::words::base::plural;

use super::types::{plain, GoldenDelta};
use crate::errors::EngineError;
use crate::golden_seam::{agent_installers, AgentInstaller, GoldenImport};

/// golden-import.ts:1396 `AGENT_INSTALLERS`: the catalog's agents by id, and aider.
pub static AGENT_INSTALLERS: LazyLock<IndexMap<String, AgentInstaller>> = LazyLock::new(|| {
    let agents: Vec<_> = catalog_agents().into_iter().cloned().collect();
    agent_installers(&agents)
});

/// golden.ts:868. An updated version's sha chains the version it came from with what the delta ran.
pub fn next_setup_sha(previous: &str, setup: &str, imp: &GoldenImport) -> String {
    hex::encode(Sha256::digest(format!("{previous}\n{}", setup_sha_of(setup, Some(imp))).as_bytes()))
}

/// golden.ts:873. Pins which installers ran: the harness line and every agent's.
pub fn setup_sha_of(setup: &str, imp: Option<&GoldenImport>) -> String {
    let mut h = Sha256::new();
    h.update(setup.as_bytes());
    if let Some(imp) = imp {
        if let Some(node) = &imp.node {
            h.update(format!("\n{}", node.cmd).as_bytes());
        }
        for a in &imp.agents {
            h.update(format!("\n{}", a.install).as_bytes());
        }
    }
    hex::encode(h.finalize())
}

const SMOKE_JOIN: &str = " && ";

/// golden.ts:1267. The agents' version checks as one command for the fork; none is `true`, so the fork still boots.
pub fn join_smoke(checks: &[String]) -> String {
    if checks.is_empty() {
        "true".to_string()
    } else {
        checks.join(SMOKE_JOIN)
    }
}

/// golden.ts:1271 `PROJECT_CHECKS`: the version checks the catalog's install rows name for the managers a project's
/// install runs.
static PROJECT_CHECKS: LazyLock<HashSet<String>> =
    LazyLock::new(|| SEED_ROWS.iter().flat_map(|row| row.installs.iter().flatten()).filter_map(|i| i.check.clone()).collect());

/// golden.ts:1275. A tools row's version check where its tool is a manager a project's install runs: one on the image
/// that does not start fails every project there, and `command -v` reads a command whose package lost its files as
/// present.
pub fn manager_check_of(id: &str) -> Option<String> {
    let entry = catalog_entry(&catalog_id_of_row(id).unwrap_or_default())?;
    let smoke = smoke_of(entry);
    PROJECT_CHECKS.contains(&smoke).then_some(smoke)
}

/// golden.ts:1279.
pub fn smoke_checks(smoke: &str) -> Vec<String> {
    smoke.split(SMOKE_JOIN).filter(|p| *p != "true").map(str::to_string).collect()
}

/// golden.ts:1285. The version checks of what is on the image after the delta: the previous smoke without the removed
/// agents', joined with the added agents'.
pub fn next_smoke(previous: &str, removed: &[String], added: &str) -> String {
    let mut parts: Vec<String> = smoke_checks(previous).into_iter().filter(|p| !removed.contains(p)).collect();
    for p in smoke_checks(added) {
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    join_smoke(&parts)
}

/// golden.ts:1293. What a passed smoke proved, for the stage's end line: each agent named by the installer table its
/// check came from, then every other check by the command it ran.
pub fn smoke_tally(smoke: &str) -> String {
    let checks = smoke_checks(smoke);
    if checks.is_empty() {
        return "no agent to check; the fork booted".to_string();
    }
    let known = |c: &String| AGENT_INSTALLERS.values().find(|i| i.smoke == *c);
    let agents: Vec<String> = checks.iter().filter_map(|c| known(c).map(|i| i.name.clone())).collect();
    let tools: Vec<&str> = checks.iter().filter(|c| known(c).is_none()).map(|c| c.split(' ').next().unwrap_or("")).collect();
    let answered = if agents.is_empty() {
        "no agent to check".to_string()
    } else {
        format!("{} {}: {}", plural(agents.len() as f64, "agent"), if agents.len() == 1 { "answers" } else { "answer" }, agents.join(", "))
    };
    if tools.is_empty() {
        answered
    } else {
        format!("{answered}; {} {}", tools.join(", "), if tools.len() == 1 { "runs" } else { "run" })
    }
}

/// golden.ts:1303. The recipe rows a delta addresses: what it retires and what it plans again, tools and agents alike.
pub fn touched_by(delta: &GoldenDelta) -> IndexSet<String> {
    let imp = &delta.import;
    delta
        .retired
        .iter()
        .map(|r| r.id.clone())
        .chain(imp.tools.iter().map(|t| t.id.clone()))
        .chain(imp.skipped_tools.iter().map(|t| t.id.clone()))
        .chain(imp.agents.iter().map(|a| a.id.clone()))
        .chain(imp.skipped_agents.iter().map(|a| a.id.clone()))
        .collect()
}

/// golden.ts:1310. What an updated image is missing: the previous version's missing tools the delta neither retired nor
/// planned again, then what this delta's tools stage skipped or failed.
pub fn next_missing(previous: &[GoldenMissingTool], delta: &GoldenDelta, fresh: &[GoldenMissingTool]) -> Vec<GoldenMissingTool> {
    let touched = touched_by(delta);
    previous.iter().filter(|p| !touched.contains(&p.id)).chain(fresh.iter()).cloned().collect()
}

/// golden.ts:1317. What an updated image is without: the previous version's notes for rows the delta neither retired
/// nor planned again, then what this delta's pack left behind.
pub fn next_left_behind(previous: &[GoldenLeftBehind], delta: &GoldenDelta, fresh: &[GoldenLeftBehind]) -> Vec<GoldenLeftBehind> {
    let touched = touched_by(delta);
    previous.iter().filter(|p| !touched.contains(&p.id)).chain(fresh.iter()).cloned().collect()
}

/// golden.ts:1471. Moves the head to a version the manifest holds; refuses one it does not.
pub fn rollback(manifest: &GoldenManifest, version: u32) -> Result<GoldenManifest, EngineError> {
    if !manifest.versions.iter().any(|v| v.version.0 == version as f64) {
        return Err(plain(format!("rollback target v{version} not in manifest")));
    }
    Ok(GoldenManifest { head: JsNumber(version as f64), ..manifest.clone() })
}
