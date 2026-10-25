// SPDX-License-Identifier: AGPL-3.0-only
//! What one project folder's own files say it needs. Every reader hands back words; naming the catalog row a word lands
//! on happens once, here. Port of packages/collect/src/project/index.ts at a3b870a6e; the spec is spec/project.md.
pub mod reader;
pub mod readers;

use indexmap::IndexMap;
use wsp_catalog::catalog::{catalog_tool_for, catalog_tool_for_dependency};

use crate::host::{Host, StatKind};
use crate::project::reader::{ProjectFile, ProjectReader, Reads};
use crate::project::readers::{marker, package_json, toolchain_pins, workflows};

/// The readers in the order their files speak plainest; the first one to name a tool writes its row's line.
pub const PROJECT_READERS: [ProjectReader; 4] =
    [package_json::PACKAGE_JSON_READER, toolchain_pins::TOOLCHAIN_PINS_READER, marker::MARKER_READER, workflows::WORKFLOWS_READER];

/// index.ts:19: one tool a project asks for, on the row it lands on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNeed {
    /// The catalog id when the catalog carries the tool, else the word the project's own file used.
    pub id: String,
    /// The catalog's name for it, else the word as its own ecosystem writes it.
    pub name: String,
    pub why: String,
}

/// index.ts:28.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectScan {
    /// The folder that was read, trailing slashes cut.
    pub dir: String,
    /// Needs the catalog carries, by catalog id, in first-found order.
    pub rows: Vec<ProjectNeed>,
    /// Needs the catalog has no row for, by word, in first-found order.
    pub candidates: Vec<ProjectNeed>,
}

/// The paths one entry of a reader's list stands for.
pub async fn paths_of(host: &Host, dir: &str, entry: &str) -> Vec<String> {
    if let Some(folder) = entry.strip_suffix('/') {
        return host.fs.list(&format!("{dir}/{folder}")).await.into_iter().map(|name| format!("{entry}{name}")).collect();
    }
    let Some(star) = entry.find("/*/") else {
        return vec![entry.to_string()];
    };
    let head = &entry[..star];
    let rest = &entry[star + 2..];
    host.fs.list(&format!("{dir}/{head}")).await.into_iter().map(|name| format!("{head}/{name}{rest}")).collect()
}

/// The files under `dir` a reader asked for that are there, in the order it asked.
pub async fn files_for(host: &Host, dir: &str, reader: &ProjectReader) -> Vec<ProjectFile> {
    let mut out = Vec::new();
    for entry in reader.files {
        for path in paths_of(host, dir, entry).await {
            let at = format!("{dir}/{path}");
            if reader.reads == Reads::Presence {
                if host.fs.stat(&at).await.is_some_and(|s| s.kind == StatKind::File) {
                    out.push(ProjectFile { path, text: String::new() });
                }
                continue;
            }
            if let Some(text) = host.fs.read_text(&at).await {
                out.push(ProjectFile { path, text });
            }
        }
    }
    out
}

/// Every reader over one project folder, its findings named against the catalog.
pub async fn scan_project(host: &Host, folder: &str) -> ProjectScan {
    let dir = folder.trim_end_matches('/');
    let mut rows: IndexMap<String, ProjectNeed> = IndexMap::new();
    let mut candidates: IndexMap<String, ProjectNeed> = IndexMap::new();
    for reader in PROJECT_READERS {
        for file in files_for(host, dir, &reader).await {
            for finding in (reader.read)(&file) {
                let entry = match finding.installs {
                    Some(road) => catalog_tool_for_dependency(road, &finding.name),
                    None => catalog_tool_for(&finding.name),
                };
                if let Some(entry) = entry {
                    rows.entry(entry.id.clone()).or_insert_with(|| ProjectNeed {
                        id: entry.id.clone(),
                        name: entry.name.clone(),
                        why: finding.why.clone(),
                    });
                    continue;
                }
                if finding.catalog_only {
                    continue;
                }
                candidates.entry(finding.name.clone()).or_insert_with(|| ProjectNeed {
                    id: finding.name.clone(),
                    name: finding.label.clone(),
                    why: finding.why.clone(),
                });
            }
        }
    }
    ProjectScan { dir: dir.to_string(), rows: rows.into_values().collect(), candidates: candidates.into_values().collect() }
}
