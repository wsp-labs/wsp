// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/helpers.ts: the pieces every wire schema shares, and the serde helpers that
//! give zod's `.catch`, `.nullable().optional()` and key order their Rust form.

use indexmap::IndexMap;
use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{Check, SchemaError};
use crate::views::harness::HarnessOption;

/// The id a request carries and its reply echoes: a string or a number as the client sent it, a fraction kept a
/// fraction (rule 4.13). zod's `reqId`, `z.union([z.string(), z.number()])`.
pub use wsp_frames::RequestId as ReqId;

/// What picking one option on a permission prompt does to the tool call in front of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionEffect {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "mode")]
    Mode,
    #[serde(rename = "answer")]
    Answer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    pub effect: PermissionEffect,
    /// The access mode the rest of the turn runs in; set on the mode effect only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// helpers.ts `permissionPrompt`: one permission prompt's own fields, which the views flatten into the transcript row
/// and the waiting question alike. Field order is the TS object's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPrompt {
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub options: Vec<PermissionOption>,
}

/// helpers.ts `portCloseDetail`: who held a port when it closed, flattened into port.close on both wires.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCloseDetail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<crate::types::JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exited: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<String>,
}

/// The refusal `listed` throws: the sentence, zod-free, stamped kind usage, with how many options were offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedRefusal {
    pub message: String,
    /// Always "usage".
    pub kind: &'static str,
    pub offered: usize,
}

/// The highest a `channel` field takes: an integer from 0 to 255.
pub const CHANNEL_MAX: i64 = 255;

/// The longest cgroup path a frame may carry, in UTF-16 units.
pub const CGROUP_PATH_MAX: usize = 512;

// helpers.ts:43
// const optionWords = (options: ReadonlyArray<HarnessOption>): string => options.map(o => `${o.label} (${o.value})`).join(", ");
//
// helpers.ts:45
// function listed(subject: string, word: string, options: ReadonlyArray<HarnessOption>, value: string | undefined, legacy: ReadonlyArray<HarnessOption> = [], hidden: ReadonlyArray<HarnessOption> = []): void {
//   if (value === undefined || options.some(o => o.value === value) || legacy.some(o => o.value === value) || hidden.some(o => o.value === value)) return;
//   const older = legacy.length === 0 ? "" : `; legacy: ${optionWords(legacy)}`;
//   const said = options.length === 0 ? `${subject} takes no ${word}` : `${word} "${value}" is not one ${subject} takes; one of: ${optionWords(options)}${older}`;
//   throw Object.assign(new Error(said), { kind: "usage", offered: options.length });
// }
/// Refuses a value that is none of the options, the legacy ones or the hidden ones, in one sentence naming the options
/// and the legacy ones (never the hidden ones). An absent value passes.
pub fn listed(
    subject: &str,
    word: &str,
    options: &[HarnessOption],
    value: Option<&str>,
    legacy: &[HarnessOption],
    hidden: &[HarnessOption],
) -> Result<(), ListedRefusal> {
    let _ = (subject, word, options, value, legacy, hidden);
    todo!()
}

// helpers.ts:68
// const watchName = z.string().regex(/^[A-Za-z0-9_-]{1,64}$/);
/// Whether a port watch's name is one a socket may give: 1 to 64 of letters, digits, `_` and `-`.
pub fn is_watch_name(name: &str) -> bool {
    let _ = name;
    todo!()
}

// helpers.ts:71
// const cgroupPath = z
//   .string()
//   .max(512)
//   .refine(path => path.startsWith("/") && path.slice(1).split("/").every(name => name !== "" && name !== "." && name !== ".."));
/// Whether a cgroup path is one /proc/[pid]/cgroup could name: at most 512 UTF-16 units, absolute, every name after the
/// leading slash non-empty and neither `.` nor `..`. The root alone, `/`, is refused (its one name is empty).
pub fn is_cgroup_path(path: &str) -> bool {
    let _ = path;
    todo!()
}

// helpers.ts:92
// function base64Bytes(text: string): number {
//   const pad = text.endsWith("==") ? 2 : text.endsWith("=") ? 1 : 0;
//   return (text.length / 4) * 3 - pad;
// }
//
// helpers.ts:99
// export const base64Length = (bytes: number): number => Math.ceil(bytes / 3) * 4;
/// How long the base64 of so many bytes is, its padding counted.
pub fn base64_length(bytes: u64) -> u64 {
    let _ = bytes;
    todo!()
}

// helpers.ts:101
// const base64 = (bytes: number) =>
//   z
//     .string()
//     .max(base64Length(bytes))
//     .regex(/^[A-Za-z0-9+/]+={0,2}$/)
//     .refine(text => text.length % 4 === 0 && base64Bytes(text) === bytes, `must be ${bytes} bytes, base64`);
/// Whether text is base64 of exactly `bytes` bytes as zod's `base64(bytes)` reads it: at most base64_length(bytes)
/// characters, the standard alphabet then up to two `=`, a length a multiple of 4, and the byte count read off the
/// text and its padding, never decoded.
pub fn is_base64_of(text: &str, bytes: u64) -> bool {
    let _ = (text, bytes);
    todo!()
}

/// Rule 4.26: a parsed or built JSON object writes its keys as JavaScript does, so keys that read as array indexes
/// (`"0"`, `"2"`, `"10"`: a canonical non-negative integer below 2^32 - 1, no leading zero, no sign) move first in
/// ascending numeric order and every other key keeps its place. Applied to this map alone, not to the maps inside it.
pub fn js_key_order(map: &mut IndexMap<String, Value>) {
    let _ = map;
    todo!()
}

/// zod `.catch(undefined)` on an optional field: a value that does not read as T reads as absent rather than failing
/// the frame. Absent and null read as absent too.
pub fn or_none<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Option<T>, D::Error> {
    let _ = d;
    let _: Option<T> = None;
    todo!()
}

/// No refinement of its own.
impl Check for PermissionOption {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks each option.
impl Check for PermissionPrompt {
    fn check(&self) -> Result<(), SchemaError> {
        self.options.check().map_err(|e| e.under("options"))?;
        Ok(())
    }
}

/// No refinement of its own.
impl Check for PortCloseDetail {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PermissionEffect {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
