// SPDX-License-Identifier: AGPL-3.0-only
//! What one project folder's own files say it needs. Every reader hands back words; naming the catalog row a word lands
//! on happens once, here. Port of packages/collect/src/project/index.ts at a3b870a6e; the spec is spec/project.md.
pub mod reader;
pub mod readers;

use crate::host::Host;
use crate::project::reader::{ProjectFile, ProjectReader};
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
// project/index.ts:37
// /** The paths one entry of a reader's list stands for: every file directly under a folder named with a trailing
//  * slash, the same file in every folder under a `*` segment, else the entry itself. */
// async function pathsOf(host: Host, dir: string, entry: string): Promise<string[]> {
//   if (entry.endsWith("/")) return (await host.fs.list(`${dir}/${entry.slice(0, -1)}`)).map(name => `${entry}${name}`);
//   const star = entry.indexOf("/*/");
//   if (star < 0) return [entry];
//   const head = entry.slice(0, star);
//   return (await host.fs.list(`${dir}/${head}`)).map(name => `${head}/${name}${entry.slice(star + 2)}`);
// }
pub async fn paths_of(host: &Host, dir: &str, entry: &str) -> Vec<String> {
    todo!()
}

/// The files under `dir` a reader asked for that are there, in the order it asked.
// project/index.ts:47
// /** The files under `dir` a reader asked for that are there, in the order it asked; a reader that reads nothing out
//  * of them is told they are there and handed no bytes. */
// async function filesFor(host: Host, dir: string, reader: ProjectReader): Promise<ProjectFile[]> {
//   const out: ProjectFile[] = [];
//   for (const entry of reader.files) {
//     for (const path of await pathsOf(host, dir, entry)) {
//       const at = `${dir}/${path}`;
//       if (reader.reads === "presence") {
//         if ((await host.fs.stat(at))?.kind === "file") out.push({ path, text: "" });
//         continue;
//       }
//       const text = await host.fs.readText(at);
//       if (text !== undefined) out.push({ path, text });
//     }
//   }
//   return out;
// }
pub async fn files_for(host: &Host, dir: &str, reader: &ProjectReader) -> Vec<ProjectFile> {
    todo!()
}

/// Every reader over one project folder, its findings named against the catalog.
// project/index.ts:65
// /** Every reader over one project folder, its findings named against the catalog. What a why line carries is what the
//  * repo's own manifests name a tool with: a tool's name, a pinned version, a version range, the manifest's own file
//  * name. No other value of a file is read, and a credential never is; the recipe this writes into stays on this
//  * computer, which is not the promise the collector's other readers make about a machine. */
// export async function scanProject(host: Host, folder: string): Promise<ProjectScan> {
//   const dir = folder.replace(/\/+$/, "");
//   const rows = new Map<string, ProjectNeed>();
//   const candidates = new Map<string, ProjectNeed>();
//   for (const reader of PROJECT_READERS) {
//     for (const file of await filesFor(host, dir, reader)) {
//       for (const finding of reader.read(file)) {
//         const entry = finding.installs !== undefined ? catalogToolForDependency(finding.installs, finding.name) : catalogToolFor(finding.name);
//         if (entry !== undefined) {
//           if (!rows.has(entry.id)) rows.set(entry.id, { id: entry.id, name: entry.name, why: finding.why });
//           continue;
//         }
//         if (finding.catalogOnly === true) continue;
//         if (!candidates.has(finding.name)) candidates.set(finding.name, { id: finding.name, name: finding.label, why: finding.why });
//       }
//     }
//   }
//   return { dir, rows: [...rows.values()], candidates: [...candidates.values()] };
// }
pub async fn scan_project(host: &Host, folder: &str) -> ProjectScan {
    todo!()
}
