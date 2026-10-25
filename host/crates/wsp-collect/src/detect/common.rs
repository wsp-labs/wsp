// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces every detector builds rows with. Port of packages/collect/src/detect/common.ts at a3b870a6e; the spec
//! is spec/common.md.
use futures::future::LocalBoxFuture;

use crate::host::Host;
use crate::manifest::{Linux, ManifestEntry, RowDefault, Rung};

// common.ts:6
// /** prior is every row the rungs before this one produced. */
// export type Detector = (host: Host, prior: readonly ManifestEntry[]) => Promise<ManifestEntry[]>;
/// `prior` is every row the rungs before this one produced.
pub type Detector = for<'a> fn(&'a Host, &'a [ManifestEntry]) -> LocalBoxFuture<'a, Vec<ManifestEntry>>;

/// The optional half of a row spec: each `None` is a key the row leaves off (common.ts:8-27).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowOptions {
    pub default: Option<RowDefault>,
    pub reason: Option<String>,
    pub group: Option<String>,
    pub required: Option<bool>,
    pub linux: Option<Linux>,
    pub version: Option<String>,
    pub arch: Option<String>,
    /// Candidates the tool rewrites while it runs; those found land on the row as volatile.
    pub volatile: Option<Vec<String>>,
    /// One line for the detail pane.
    pub detail: Option<String>,
}

// common.ts:8
// export interface RowSpec {
//   rung: Rung;
//   id: string;
//   label: string;
//   /** `~/`-relative candidates; only the ones that exist end up on the row. */
//   paths: readonly string[];
//   default?: Default;
//   reason?: string;
//   group?: string;
//   required?: boolean;
//   linux?: Linux;
//   version?: string;
//   arch?: string;
//   /** Candidates the tool rewrites while it runs; those found land on the row as volatile. */
//   volatile?: readonly string[];
//   /** Emit the row even when none of the candidate paths exist. */
//   always?: boolean;
//   /** One line for the detail pane. */
//   detail?: string;
// }
#[derive(Debug, Clone, PartialEq)]
pub struct RowSpec {
    pub rung: Rung,
    pub id: String,
    pub label: String,
    /// `~/`-relative candidates; only the ones that exist end up on the row.
    pub paths: Vec<String>,
    /// Emit the row even when none of the candidate paths exist (TS `always?: boolean`, absent is false).
    pub always: bool,
    pub opt: RowOptions,
}

// common.ts:57
// export interface EntrySpec extends Omit<RowSpec, "paths" | "always"> {
//   paths: string[];
//   bytes: number;
// }
#[derive(Debug, Clone, PartialEq)]
pub struct EntrySpec {
    pub rung: Rung,
    pub id: String,
    pub label: String,
    pub paths: Vec<String>,
    pub bytes: u64,
    pub opt: RowOptions,
}

// common.ts:29
// export interface Found {
//   paths: string[];
//   bytes: number;
// }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub paths: Vec<String>,
    pub bytes: u64,
}

// common.ts:35
// /** The subset of candidates that exist, with their total size. */
// export async function found(host: Host, paths: readonly string[]): Promise<Found> {
//   const out: Found = { paths: [], bytes: 0 };
//   for (const p of paths) {
//     const s = await host.fs.stat(expand(host, p));
//     if (s === undefined) continue;
//     out.paths.push(p);
//     out.bytes += s.bytes;
//   }
//   return out;
// }
/// The subset of candidates that exist, in the order given, with their total size.
pub async fn found(host: &Host, paths: &[String]) -> Found {
    todo!()
}

// common.ts:46
// export async function exists(host: Host, path: string): Promise<boolean> {
//   return (await host.fs.stat(expand(host, path))) !== undefined;
// }
pub async fn exists(host: &Host, path: &str) -> bool {
    todo!()
}

// common.ts:51
// /** One manifest row for whichever of the spec's paths exist; undefined when none do. */
// export async function row(host: Host, spec: RowSpec): Promise<ManifestEntry | undefined> {
//   const f = await found(host, spec.paths);
//   if (f.paths.length === 0 && spec.always !== true) return undefined;
//   return entry({ ...spec, ...f });
// }
/// One manifest row for whichever of the spec's paths exist; `None` when none do and `always` is false.
pub async fn row(host: &Host, spec: RowSpec) -> Option<ManifestEntry> {
    todo!()
}

// common.ts:63
// /** Optional fields are left off rather than set to undefined so equality and snapshots stay exact. */
// export function entry(spec: EntrySpec): ManifestEntry {
//   const volatile = (spec.volatile ?? []).filter(p => spec.paths.includes(p));
//   return {
//     rung: spec.rung,
//     id: spec.id,
//     label: spec.label,
//     ...(spec.group !== undefined ? { group: spec.group } : {}),
//     paths: spec.paths,
//     bytes: spec.bytes,
//     default: spec.default ?? "bring",
//     ...(spec.reason !== undefined ? { reason: spec.reason } : {}),
//     ...(spec.required !== undefined ? { required: spec.required } : {}),
//     ...(spec.linux !== undefined ? { linux: spec.linux } : {}),
//     ...(spec.version !== undefined ? { version: spec.version } : {}),
//     ...(spec.arch !== undefined ? { arch: spec.arch } : {}),
//     ...(volatile.length > 0 ? { volatile } : {}),
//     ...(spec.detail !== undefined ? { detail: spec.detail } : {}),
//   };
// }
/// Optional fields are left off rather than set, so equality and snapshots stay exact.
pub fn entry(spec: EntrySpec) -> ManifestEntry {
    todo!()
}

// common.ts:84
// /** A list item (a formula, an extension, a global package): no source path, nothing to upload. */
// export function item(spec: Omit<EntrySpec, "paths" | "bytes">): ManifestEntry {
//   return entry({ ...spec, paths: [], bytes: 0 });
// }
/// A list item (a formula, an extension, a global package): no source path, nothing to upload.
pub fn item(rung: Rung, id: &str, label: &str, opt: RowOptions) -> ManifestEntry {
    todo!()
}

// common.ts:88
// export function present(rows: (ManifestEntry | undefined)[]): ManifestEntry[] {
//   return rows.filter((r): r is ManifestEntry => r !== undefined);
// }
pub fn present(rows: Vec<Option<ManifestEntry>>) -> Vec<ManifestEntry> {
    todo!()
}

// common.ts:92
// export function firstLine(out: string | undefined): string | undefined {
//   const line = out?.split("\n")[0]?.trim();
//   return line === undefined || line === "" ? undefined : line;
// }
/// The first line of a command's output, trimmed as JavaScript trims; `None` for no output or an empty first line.
pub fn first_line(out: Option<&str>) -> Option<String> {
    todo!()
}
