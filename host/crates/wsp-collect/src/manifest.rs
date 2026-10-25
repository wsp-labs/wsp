// SPDX-License-Identifier: AGPL-3.0-only
//! One row per thing the collector found on the computer that could be brought to an image. wsp init reads it, the
//! person ticks rows, and the same shape with `bring` and `choice` filled in is saved as the recipe file. Port of
//! packages/collect/src/manifest.ts at a3b870a6e; the spec is spec/manifest.md.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_wire::types::init_job::LoginChoice;
use wsp_wire::types::recipe_file::ServerSignIn;
use wsp_wire::views::golden_image::ToolPin;
use wsp_words::exit::HostError;

pub use wsp_wire::types::init_job::LOGIN_CHOICES;

// manifest.ts:9
// export const RUNGS = ["identity", "shell", "toolchains", "tools", "agents", "logins"] as const;
// export const Rung = z.enum(RUNGS);
// export type Rung = z.infer<typeof Rung>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rung {
    Identity,
    Shell,
    Toolchains,
    Tools,
    Agents,
    Logins,
}

/// The ladder, in the order collect() runs it and a manifest lists it.
pub const RUNGS: [Rung; 6] = [Rung::Identity, Rung::Shell, Rung::Toolchains, Rung::Tools, Rung::Agents, Rung::Logins];

impl Rung {
    /// The word the schema and an id prefix use: `identity`, `shell`, ...
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

/// TS `Default` (manifest.ts:15), renamed: the TS name shadows the prelude's `Default` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowDefault {
    Bring,
    Skip,
}

// manifest.ts:18
// /** Whether a tools row can be installed on the Linux machine; unknown when nothing on the laptop can tell. */
// export const LINUX = ["yes", "no", "unknown"] as const;
// export const Linux = z.enum(LINUX);
// export type Linux = z.infer<typeof Linux>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Linux {
    Yes,
    No,
    Unknown,
}

pub const LINUX: [Linux; 3] = [Linux::Yes, Linux::No, Linux::Unknown];

/// manifest.ts:23-69. Fields in the schema's key order, which is the order zod rebuilds a parsed row in, so the order
/// collect() answers and the recipe file saves; `arch` is the one key the schema lacks (detect/common.ts:77 writes it,
/// parse_manifest drops it as zod's strip does).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub rung: Rung,
    /// `<rung>/<name>`; the last segment looks up install and sign-in commands.
    pub id: String,
    pub label: String,
    /// Source paths on the computer, `~`-relative; empty for a list item.
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excludes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatile: Option<Vec<String>>,
    pub bytes: u64,
    pub default: RowDefault,
    /// Why the default is skip; with a reason the row renders locked off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bring: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Not in the schema: a detector's row may carry it, a parsed row never does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<ServerSignIn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
}

impl ManifestEntry {
    /// A row with only the required fields set and every optional absent, `default` bring.
    pub fn new(rung: Rung, id: impl Into<String>, label: impl Into<String>, paths: Vec<String>, bytes: u64) -> ManifestEntry {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

/// One zod issue as manifest.ts's schemas raise it: the path (keys as strings, indexes as numbers) and zod's message.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestIssue {
    pub path: Vec<Value>,
    pub message: String,
}

/// `ManifestEntry.safeParse(data)` (manifest.ts:71-96): the row, or every issue zod raises for it, in zod's order
/// (spec/manifest.md, rules 3 to 6).
// manifest.ts:71
// export const ManifestEntry = Fields.superRefine((e, ctx) => {
//   if (!e.id.startsWith(`${e.rung}/`)) {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["id"], message: `id must start with ${e.rung}/` });
//   }
//   if (e.choice !== undefined && e.rung !== "logins" && e.consent !== true) {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["choice"], message: "only a logins row or a consent row carries a choice" });
//   }
//   if (e.linux !== undefined && e.rung !== "tools") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["linux"], message: "only a tools row carries a linux marker" });
//   }
//   if (e.version !== undefined && e.rung !== "tools") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["version"], message: "only a tools row carries a version" });
//   }
//   if (e.login !== undefined && e.rung !== "shell") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["login"], message: "only a shell row carries a login shell" });
//   }
//   if (e.font !== undefined && e.rung !== "shell") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["font"], message: "only a shell row carries a terminal font" });
//   }
//   if (e.sources !== undefined && e.rung !== "shell") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["sources"], message: "only a shell row carries sourced files" });
//   }
//   if (e.required === true && e.default === "skip") {
//     ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["required"], message: "a required row cannot default to skip" });
//   }
// });
// export type ManifestEntry = z.infer<typeof ManifestEntry>;
pub fn parse_entry(data: &Value) -> Result<ManifestEntry, Vec<ManifestIssue>> {
    todo!()
}

/// `Manifest.safeParse(data)` (manifest.ts:99-107): the envelope, every row and the duplicate-id check.
// manifest.ts:99
// export const Manifest = z.object({ entries: z.array(ManifestEntry) }).superRefine((m, ctx) => {
//   const seen = new Set<string>();
//   m.entries.forEach((e, i) => {
//     if (seen.has(e.id)) {
//       ctx.addIssue({ code: z.ZodIssueCode.custom, path: ["entries", i, "id"], message: `duplicate id ${e.id}` });
//     }
//     seen.add(e.id);
//   });
// });
// export type Manifest = z.infer<typeof Manifest>;
pub fn safe_parse_manifest(data: &Value) -> Result<Manifest, Vec<ManifestIssue>> {
    todo!()
}

/// Parses unknown data into a manifest; the error is `invalid manifest: ` and every issue as `issuesLine` writes it,
/// `entries.3.rung: <message>` joined by `; ` (manifest.ts:111).
// manifest.ts:110
// /** Parses unknown data into a manifest; the error names the failing row and field (entries.3.rung). */
// export function parseManifest(data: unknown): Manifest {
//   const r = Manifest.safeParse(data);
//   if (r.success) return r.data;
//   throw new Error(`invalid manifest: ${issuesLine(r.error.issues)}`);
// }
pub fn parse_manifest(data: &Value) -> Result<Manifest, HostError> {
    todo!()
}
