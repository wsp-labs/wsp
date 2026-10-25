// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/thread-defaults.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use indexmap::IndexMap;

/// TS: packages/protocol/src/thread-defaults.ts:18 at a3b870a6e.
pub const ACCESS_CHOICES: &[&str] = &["ask", "auto-edit", "full", "plan"];

/// TS: packages/protocol/src/thread-defaults.ts:307 at a3b870a6e.
pub const BUILT_IN_LIST_CLAUSE: &str = "; that list is wsp's built-in one, since no agent there described itself, and the agent on that computer may take more";

/// TS: packages/protocol/src/thread-defaults.ts:309 at a3b870a6e.
pub const BUILT_IN_TABLE_CLAUSE: &str = "; wsp's built-in table says so, since no agent there described itself";

/// TS: packages/protocol/src/thread-defaults.ts:394 at a3b870a6e.
pub const ENV_REFUSED_FIX: &str = "Name another variable; this one is the computer's to say.";

/// TS: packages/protocol/src/thread-defaults.ts:439 at a3b870a6e.
pub const ENV_VALUE_REFUSAL: &str = "only a socket holding this host's own token may set a variable's value; use wsp agents setup at the computer the host runs on";

/// TS: packages/protocol/src/thread-defaults.ts:19 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessChoice {
    #[serde(rename = "ask")]
    Ask,
    #[serde(rename = "auto-edit")]
    AutoEdit,
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "plan")]
    Plan,
}

impl Check for AccessChoice {}

/// TS: packages/protocol/src/thread-defaults.ts:24 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPicker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<Vec<String>>,
}

impl Check for ModelPicker {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<ModelPicker>,
}

impl Check for AgentDefaults {
    /// Nested checks: access, models.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:30 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDefaultsPatch {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub model: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub effort: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub access: Option<Option<AccessChoice>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub models: Option<Option<ModelPicker>>,
}

impl Check for AgentDefaultsPatch {
    /// Nested checks: access, models.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:40 at a3b870a6e.
pub type TrackerRepo = String;
// Rules: TrackerRepo: matches /^[^/\s]+\/[^/\s]+$/.

/// TS: packages/protocol/src/thread-defaults.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<TrackerRepo>>,
}

impl Check for ProjectOverrides {
    /// Rules: afterWorktree: zod string check trim; afterWorktree: at least 1 UTF-16 units.
    /// Nested checks: access, trackers.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOverridesPatch {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub agent: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub model: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub effort: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub access: Option<Option<AccessChoice>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub after_worktree: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub trackers: Option<Option<Vec<TrackerRepo>>>,
}

impl Check for ProjectOverridesPatch {
    /// Rules: afterWorktree: zod string check trim; afterWorktree: at least 1 UTF-16 units.
    /// Nested checks: access, trackers.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:58 at a3b870a6e.
pub type EnvName = String;
// Rules: EnvName: matches /^[A-Za-z_][A-Za-z0-9_]*$/.

/// TS: packages/protocol/src/thread-defaults.ts:63 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSetupView {
    pub on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    pub env_names: Vec<String>,
}

impl Check for AgentSetupView {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:69 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSetupSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub program: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub config_dir: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub args: Option<Option<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, Option<String>>>,
}

impl Check for AgentSetupSet {
    /// Rules: program: at least 1 UTF-16 units; configDir: at least 1 UTF-16 units.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:121 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefaultFrom {
    #[serde(rename = "named")]
    Named,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "catalog")]
    Catalog,
}

impl Check for DefaultFrom {}

/// TS: packages/protocol/src/thread-defaults.ts:123 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPick {
    pub value: String,
    pub from: DefaultFrom,
}

impl Check for ResolvedPick {
    /// Nested checks: from.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:126 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAccess {
    pub value: AccessChoice,
    pub mode: String,
    pub from: DefaultFrom,
}

impl Check for ResolvedAccess {
    /// Nested checks: value, from.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:130 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDefaults {
    pub agent: ResolvedPick,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ResolvedPick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ResolvedPick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<ResolvedAccess>,
}

impl Check for ThreadDefaults {
    /// Nested checks: agent, model, effort, access.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// ---- Hand-written below: the functions of the unit.

// thread-defaults.ts:48 at a3b870a6e
// export function patchedFields<T extends object>(kept: T | undefined, patch: { [K in keyof T]?: T[K] | null }): T | undefined {
//   const next: Record<string, unknown> = { ...kept };
//   for (const [key, value] of Object.entries(patch)) {
//     if (value === null) delete next[key];
//     else if (value !== undefined) next[key] = value;
//   }
//   return Object.keys(next).length === 0 ? undefined : (next as T);
// }
/// A record with a patch's fields over it, read as JSON objects: a key the patch writes null is taken away, an absent
/// one kept, any other value set (a key the record lacked goes after the ones it had). None when no key is left.
/// `T` is the record (AgentDefaults, ProjectOverrides), `P` its patch (AgentDefaultsPatch, ProjectOverridesPatch),
/// whose fields are double options so null and absent stay apart.
pub fn patched_fields<T, P>(kept: Option<&T>, patch: &P) -> Option<T>
where
    T: Serialize + serde::de::DeserializeOwned,
    P: Serialize,
{
    todo!()
}

// thread-defaults.ts:81 at a3b870a6e
// export interface AgentSetup {
//   on?: false;
//   program?: string;
//   configDir?: string;
//   args?: string[];
//   env?: Record<string, string>;
// }
/// One agent's launch setup as the host keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<wsp_frames::False>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<indexmap::IndexMap<String, String>>,
}

// thread-defaults.ts:411 at a3b870a6e
// export interface ResolvedFolder {
//   folder: string;
//   home: string;
//   kept: readonly string[];
// }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFolder {
    pub folder: String,
    pub home: String,
    pub kept: Vec<String>,
}
