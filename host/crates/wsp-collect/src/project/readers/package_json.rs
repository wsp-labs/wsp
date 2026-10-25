// SPDX-License-Identifier: AGPL-3.0-only
//! package.json: the manager it pins, the runtimes its engines field accepts, the programs its scripts run and the
//! packages it depends on. Port of packages/collect/src/project/readers/package-json.ts at a3b870a6e; the spec is
//! spec/project.md.
use crate::project::reader::{ProjectFile, ProjectFinding, ProjectReader, Reads};

pub const FILES: [&str; 3] = ["package.json", "apps/*/package.json", "packages/*/package.json"];

// project/readers/package-json.ts:18
// function parse(text: string): PackageJson | undefined {
//   try {
//     const value: unknown = JSON.parse(text);
//     return typeof value === "object" && value !== null ? (value as PackageJson) : undefined;
//   } catch {
//     return undefined;
//   }
// }
//
// const strings = (value: unknown): [string, string][] =>
//   typeof value === "object" && value !== null ? Object.entries(value).flatMap(([k, v]) => (typeof v === "string" ? [[k, v] as [string, string]] : [])) : [];
//
// export const packageJsonReader: ProjectReader = {
//   id: "package-json",
//   // A workspace keeps its browser or its linters in a package's own manifest, not the root's.
//   files: ["package.json", "apps/*/package.json", "packages/*/package.json"],
//   reads: "text",
//   read(file): readonly ProjectFinding[] {
//     const pkg = parse(file.text);
//     if (pkg === undefined) return [];
//     const out: ProjectFinding[] = [];
//     if (typeof pkg.packageManager === "string" && pkg.packageManager !== "") {
//       // corepack writes a `+sha512.<88 chars>` integrity suffix after the version; the line the row shows drops it.
//       const spec = pkg.packageManager.split("+")[0] ?? pkg.packageManager;
//       const name = spec.split("@")[0] ?? spec;
//       if (name !== "") out.push({ name, label: name, why: `packageManager ${spec}` });
//     }
//     for (const [key, range] of strings(pkg.engines)) out.push({ name: key, label: key, why: `engines.${key} ${range}`, catalogOnly: true });
//     for (const [, command] of strings(pkg.scripts)) {
//       for (const name of commandNames(command)) out.push({ name, label: name, why: `${file.path} scripts run ${name}`, catalogOnly: true });
//     }
//     for (const [name, range] of [...strings(pkg.dependencies), ...strings(pkg.devDependencies)]) out.push({ name, label: name, why: `${file.path} depends on ${name} ${range}`, catalogOnly: true, installs: "npm" });
//     return out;
//   },
// };
pub fn read(file: &ProjectFile) -> Vec<ProjectFinding> {
    todo!()
}

pub const PACKAGE_JSON_READER: ProjectReader = ProjectReader { id: "package-json", files: &FILES, reads: Reads::Text, read };
