// SPDX-License-Identifier: AGPL-3.0-only
//! What every reader of a project's own files hands back, and the one shape a reader takes. A reader knows one kind of
//! manifest and none of the catalog's rows. Port of packages/collect/src/project/reader.ts at a3b870a6e; the spec is
//! spec/project.md.
use wsp_catalog::roads::RoadName;

/// reader.ts:9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFinding {
    /// The word the file used: a command, a package name, a language, a manager.
    pub name: String,
    /// That word as the file's own ecosystem writes it.
    pub label: String,
    /// The line the row shows, naming the file that asked and what it said.
    pub why: String,
    /// Evidence only where the catalog already carries the name (TS `catalogOnly?: true`).
    pub catalog_only: bool,
    /// The road the project's own install puts this package on the machine by.
    pub installs: Option<RoadName>,
}

/// reader.ts:26.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFile {
    /// Relative to the project folder, slash-separated.
    pub path: String,
    pub text: String,
}

/// reader.ts:38.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reads {
    Text,
    Presence,
}

/// reader.ts:33: one kind of project manifest, read into the tools it names.
#[derive(Debug, Clone, Copy)]
pub struct ProjectReader {
    pub id: &'static str,
    /// Paths under the project folder, in the order it wants them; `name/` is every file directly under that folder,
    /// and a `*` segment is every folder at that depth.
    pub files: &'static [&'static str],
    pub reads: Reads,
    pub read: fn(&ProjectFile) -> Vec<ProjectFinding>,
}
