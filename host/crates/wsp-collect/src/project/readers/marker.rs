// SPDX-License-Identifier: AGPL-3.0-only
//! The manifests whose presence is the whole signal: a lockfile, a module file, a compose file. Port of
//! packages/collect/src/project/readers/marker.ts at a3b870a6e; the spec is spec/project.md.
use crate::project::reader::{ProjectFile, ProjectFinding, ProjectReader, Reads};

/// marker.ts:7, in this order: file, then (name, label).
pub const MARKERS: [(&str, &str, &str); 20] = [
    ("pnpm-lock.yaml", "pnpm", "pnpm"),
    ("yarn.lock", "yarn", "Yarn"),
    ("bun.lock", "bun", "Bun"),
    ("bun.lockb", "bun", "Bun"),
    ("package-lock.json", "npm", "npm"),
    ("pyproject.toml", "python", "Python"),
    ("uv.lock", "uv", "uv"),
    ("requirements.txt", "python", "Python"),
    (".tool-versions", "mise", "mise"),
    ("mise.toml", "mise", "mise"),
    (".mise.toml", "mise", "mise"),
    ("go.mod", "go", "Go"),
    ("Cargo.toml", "rust", "Rust"),
    ("Gemfile", "ruby", "Ruby"),
    ("composer.json", "php", "PHP"),
    ("Dockerfile", "docker", "Docker"),
    ("compose.yaml", "docker", "Docker"),
    ("compose.yml", "docker", "Docker"),
    ("docker-compose.yaml", "docker", "Docker"),
    ("docker-compose.yml", "docker", "Docker"),
];

/// The files MARKERS names, in its order.
pub const MARKER_FILES: [&str; 20] = [
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "package-lock.json",
    "pyproject.toml",
    "uv.lock",
    "requirements.txt",
    ".tool-versions",
    "mise.toml",
    ".mise.toml",
    "go.mod",
    "Cargo.toml",
    "Gemfile",
    "composer.json",
    "Dockerfile",
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// One finding `<path> needs <label>` for a path MARKERS names, else none.
// project/readers/marker.ts:30
// export const markerReader: ProjectReader = {
//   id: "marker",
//   files: Object.keys(MARKERS),
//   reads: "presence",
//   read(file): readonly ProjectFinding[] {
//     const marker = MARKERS[file.path];
//     return marker === undefined ? [] : [{ ...marker, why: `${file.path} needs ${marker.label}` }];
//   },
// };
pub fn read(file: &ProjectFile) -> Vec<ProjectFinding> {
    todo!()
}

pub const MARKER_READER: ProjectReader = ProjectReader { id: "marker", files: &MARKER_FILES, reads: Reads::Presence, read };
