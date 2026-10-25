// SPDX-License-Identifier: AGPL-3.0-only
//! Three shapes the host writes in an order its code builds, not the schema's: a backend's capabilities write `sizes` after
//! `ownNetwork`, and a plugin row writes `project`, `path` and `setIn` after `brings`, and a harness catalog row writes `source`, `version`, `contextWindows`,
//! `steers`, `renames` and `images` before `harness` (harness-catalog.ts `fromTable`). Read and written back, the text is
//! the input's byte for byte.
mod common;
use common::reads_exact;
use serde_json::json;
use wsp_wire::types::agents_report::PluginRow;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::wire::capabilities::Capabilities;

#[test]
fn a_boxs_capabilities_write_sizes_last() {
    let box_capabilities = json!({
        "liveCloneForks": false, "pauseMode": "disk", "replacesMachine": true, "previewUrls": true, "signedUrls": false,
        "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": true, "snapshotListing": true,
        "templates": true, "kept": false, "copies": true, "ownNetwork": true,
        "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.11, "name": "small" }]
    });
    let read: Capabilities = reads_exact(box_capabilities);
    assert_eq!(read.sizes.len(), 1);
}

#[test]
fn a_plugin_row_writes_project_path_and_set_in_after_brings() {
    let row = json!({
        "agent": "claude", "id": "frontend-design@official", "name": "frontend-design", "marketplace": "official", "version": "1.2.0",
        "scope": "local", "on": true, "missing": "folder", "description": "d", "source": "anthropics/claude-code",
        "brings": { "skills": ["frontend-design"], "commands": [], "subagents": [], "hooks": [], "servers": [], "lsp": [], "apps": [] },
        "project": { "id": "pr_1a2b3c4d", "name": "wsp", "path": "~/wsp" }, "path": "~/.claude/plugins/cache/official/frontend-design",
        "setIn": "~/wsp/.claude/settings.local.json"
    });
    let read: PluginRow = reads_exact(row);
    assert_eq!(read.set_in.as_deref(), Some("~/wsp/.claude/settings.local.json"));
}

#[test]
fn a_catalog_row_writes_its_table_keys_before_harness() {
    let row = json!({
        "source": "table", "version": null, "contextWindows": [], "steers": false, "renames": false, "images": false,
        "harness": "gemini", "label": "Gemini CLI", "models": [], "efforts": [], "bypassMode": "yolo", "permissionModes": []
    });
    let read: HarnessCatalog = reads_exact(row);
    assert_eq!(read.harness, "gemini");
}
