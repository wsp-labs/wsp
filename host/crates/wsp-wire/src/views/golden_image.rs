// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/golden-image.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::init_job::LoginChoice;
use crate::types::init_job::LoginState;
use crate::types::js_number::JsNumber;
use wsp_frames::True;
use crate::wire::capabilities::WorkspaceSize;

literal!(CustomLit, "custom");
literal!(LitV1, 1);
literal!(WspVault1Lit, "wsp-vault-1");
literal!(Aes256GcmLit, "aes-256-gcm");

/// TS: packages/protocol/src/views/golden-image.ts:93 at a3b870a6e.
pub const AGENTS_BIN: &str = "/opt/wsp-agents/bin";

/// TS: packages/protocol/src/views/golden-image.ts:203 at a3b870a6e.
pub const RECIPE_TICKS: &[&str] = &["used", "installed", "default"];

/// TS: packages/protocol/src/views/golden-image.ts:255 at a3b870a6e.
pub const ADDED_BY_AGENT: &str = "added by the agent";

/// TS: packages/protocol/src/views/golden-image.ts:419 at a3b870a6e.
pub const IMAGE_PASSPHRASE_MIN: u64 = 12;

/// TS: packages/protocol/src/views/golden-image.ts:438 at a3b870a6e.
pub const ALREADY_APPLIED: &str = "already applied";

/// TS: packages/protocol/src/views/golden-image.ts:440 at a3b870a6e.
pub const MCP_ID_PREFIX: &str = "agents/mcp/";

/// TS: packages/protocol/src/views/golden-image.ts:453 at a3b870a6e.
pub const BREW_ID_PREFIX: &str = "tools/brew/";

/// TS: packages/protocol/src/views/golden-image.ts:10 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MachineKind;

impl Check for MachineKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["sandbox", "desktop"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenLogin {
    pub name: String,
    pub state: LoginState,
}

impl Check for GoldenLogin {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<LoginState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "state" => <LoginState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoldenMissingToolOutcome {
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for GoldenMissingToolOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["skipped", "failed"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:18 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenMissingTool {
    pub id: String,
    pub name: String,
    pub outcome: GoldenMissingToolOutcome,
    pub note: String,
}

impl Check for GoldenMissingTool {
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<GoldenMissingToolOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        match obj.get("note") { None => out.push(crate::types::check::missing::<Self>("note")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "outcome" => <GoldenMissingToolOutcome as Check>::describe(rest),
                "note" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenLeftBehind {
    pub id: String,
    pub path: String,
    pub note: String,
}

impl Check for GoldenLeftBehind {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("note") { None => out.push(crate::types::check::missing::<Self>("note")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "note" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBaseTool {
    pub name: String,
    pub version: String,
}

impl Check for GoldenBaseTool {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "version" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:30 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenRetired {
    pub id: String,
    pub name: String,
}

impl Check for GoldenRetired {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenVersionSmoke {
    pub cmd: String,
    pub exit_code: JsNumber,
}

impl Check for GoldenVersionSmoke {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cmd") { None => out.push(crate::types::check::missing::<Self>("cmd")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cmd"))), } } }
        match obj.get("exitCode") { None => out.push(crate::types::check::missing::<Self>("exitCode")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cmd" => <String as Check>::describe(rest),
                "exitCode" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:36 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenVersion {
    pub version: JsNumber,
    pub snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub template_id: Option<String>,
    pub base_template: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<MachineKind>,
    pub setup_sha: String,
    pub created_at: String,
    pub smoke: GoldenVersionSmoke,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub browser_shim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub npm_bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logins: Option<Vec<GoldenLogin>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub missing_tools: Option<Vec<GoldenMissingTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub silenced: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shell_noise: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub left_behind: Option<Vec<GoldenLeftBehind>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base: Option<Vec<GoldenBaseTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub retired: Option<Vec<GoldenRetired>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub used_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub image_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub no_secrets: Option<bool>,
}

impl Check for GoldenVersion {
    /// Rules: usedBytes: >= 0; imageHash: exactly 64 UTF-16 units.
    /// Nested checks: kind, smoke, size, logins, missingTools, leftBehind, base, retired.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.smoke.check().map_err(|e| e.under("smoke"))?;
        self.logins.check().map_err(|e| e.under("logins"))?;
        self.missing_tools.check().map_err(|e| e.under("missingTools"))?;
        self.left_behind.check().map_err(|e| e.under("leftBehind"))?;
        self.base.check().map_err(|e| e.under("base"))?;
        self.retired.check().map_err(|e| e.under("retired"))?;
        if let Some(v) = self.used_bytes {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("usedBytes"))?;
        }
        if let Some(v) = &self.image_hash {
            crate::types::check::text_exactly(v, 64).map_err(|e| e.under("imageHash"))?;
        }
        self.size.check().map_err(|e| e.under("size"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        match obj.get("snapshotId") { None => out.push(crate::types::check::missing::<Self>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
        if let Some(v) = obj.get("templateId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("templateId"))), } }
        match obj.get("baseTemplate") { None => out.push(crate::types::check::missing::<Self>("baseTemplate")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("baseTemplate"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<MachineKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("setupSha") { None => out.push(crate::types::check::missing::<Self>("setupSha")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setupSha"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("smoke") { None => out.push(crate::types::check::missing::<Self>("smoke")), Some(v) => { match crate::types::check::read::<GoldenVersionSmoke>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("smoke"))), } } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("browserShim") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("browserShim"))), } }
        if let Some(v) = obj.get("npmBin") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("npmBin"))), } }
        if let Some(v) = obj.get("logins") { match crate::types::check::read_present::<Vec<GoldenLogin>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("logins"))), } }
        if let Some(v) = obj.get("missingTools") { match crate::types::check::read_present::<Vec<GoldenMissingTool>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("missingTools"))), } }
        if let Some(v) = obj.get("silenced") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("silenced"))), } }
        if let Some(v) = obj.get("shellNoise") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("shellNoise"))), } }
        if let Some(v) = obj.get("leftBehind") { match crate::types::check::read_present::<Vec<GoldenLeftBehind>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("leftBehind"))), } }
        if let Some(v) = obj.get("parentSnapshotId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentSnapshotId"))), } }
        if let Some(v) = obj.get("base") { match crate::types::check::read_present::<Vec<GoldenBaseTool>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } }
        if let Some(v) = obj.get("retired") { match crate::types::check::read_present::<Vec<GoldenRetired>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("retired"))), } }
        if let Some(v) = obj.get("usedBytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_used_bytes(&x) { out.push(e.under("usedBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("usedBytes"))), } }
        if let Some(v) = obj.get("imageHash") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_image_hash(&x) { out.push(e.under("imageHash")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("imageHash"))), } }
        if let Some(v) = obj.get("noSecrets") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("noSecrets"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "version" => <JsNumber as Check>::describe(rest),
                "snapshotId" => <String as Check>::describe(rest),
                "templateId" => <Option<String> as Check>::describe(rest),
                "baseTemplate" => <String as Check>::describe(rest),
                "kind" => <Option<MachineKind> as Check>::describe(rest),
                "setupSha" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "smoke" => <GoldenVersionSmoke as Check>::describe(rest),
                "size" => None,
                "browserShim" => <Option<bool> as Check>::describe(rest),
                "npmBin" => <Option<String> as Check>::describe(rest),
                "logins" => <Option<Vec<GoldenLogin>> as Check>::describe(rest),
                "missingTools" => <Option<Vec<GoldenMissingTool>> as Check>::describe(rest),
                "silenced" => <Option<Vec<String>> as Check>::describe(rest),
                "shellNoise" => <Option<String> as Check>::describe(rest),
                "leftBehind" => <Option<Vec<GoldenLeftBehind>> as Check>::describe(rest),
                "parentSnapshotId" => <Option<String> as Check>::describe(rest),
                "base" => <Option<Vec<GoldenBaseTool>> as Check>::describe(rest),
                "retired" => <Option<Vec<GoldenRetired>> as Check>::describe(rest),
                "usedBytes" => <Option<i64> as Check>::describe(rest),
                "imageHash" => <Option<String> as Check>::describe(rest),
                "noSecrets" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GoldenVersion {
    fn rule_used_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_image_hash(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_exactly(v, 64)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:99 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenManifest {
    pub head: JsNumber,
    pub versions: Vec<GoldenVersion>,
}

impl Check for GoldenManifest {
    /// Nested checks: versions.
    fn check(&self) -> Result<(), SchemaError> {
        self.versions.check().map_err(|e| e.under("versions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("head") { None => out.push(crate::types::check::missing::<Self>("head")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } } }
        match obj.get("versions") { None => out.push(crate::types::check::missing::<Self>("versions")), Some(v) => { match crate::types::check::read::<Vec<GoldenVersion>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("versions"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "head" => <JsNumber as Check>::describe(rest),
                "versions" => <Vec<GoldenVersion> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolPinArch {
    #[serde(rename = "x86_64")]
    X8664,
    #[serde(rename = "aarch64")]
    Aarch64,
}

impl Check for ToolPinArch {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["x86_64", "aarch64"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:126 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPin {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub arch: Option<ToolPinArch>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub latest: Option<True>,
}

impl Check for ToolPin {
    /// Rules: tag: at least 1 UTF-16 units; sha256: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.tag, 1).map_err(|e| e.under("tag"))?;
        if let Some(v) = &self.sha256 {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("sha256"))?;
        }
        self.arch.check().map_err(|e| e.under("arch"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("tag") { None => out.push(crate::types::check::missing::<Self>("tag")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_tag(&x) { out.push(e.under("tag")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("tag"))), } } }
        if let Some(v) = obj.get("sha256") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_sha256(&x) { out.push(e.under("sha256")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sha256"))), } }
        if let Some(v) = obj.get("arch") { match crate::types::check::read_present::<ToolPinArch>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("arch"))), } }
        if let Some(v) = obj.get("latest") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("latest"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "tag" => <String as Check>::describe(rest),
                "sha256" => <Option<String> as Check>::describe(rest),
                "arch" => <Option<ToolPinArch> as Check>::describe(rest),
                "latest" => None,
                _ => None,
            },
        }
    }
}
impl ToolPin {
    fn rule_tag(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_sha256(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigestTicksItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub choice: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pin: Option<ToolPin>,
}

impl Check for RecipeDigestTicksItem {
    /// Nested checks: choice, pin.
    fn check(&self) -> Result<(), SchemaError> {
        self.choice.check().map_err(|e| e.under("choice"))?;
        self.pin.check().map_err(|e| e.under("pin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("choice") { match crate::types::check::read_present::<LoginChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("choice"))), } }
        if let Some(v) = obj.get("version") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } }
        if let Some(v) = obj.get("road") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } }
        if let Some(v) = obj.get("installer") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installer"))), } }
        if let Some(v) = obj.get("pin") { match crate::types::check::read_present::<ToolPin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pin"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "choice" => <Option<LoginChoice> as Check>::describe(rest),
                "version" => <Option<String> as Check>::describe(rest),
                "road" => <Option<String> as Check>::describe(rest),
                "installer" => <Option<String> as Check>::describe(rest),
                "pin" => <Option<ToolPin> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigestFilesItem {
    pub id: String,
    pub path: String,
    pub dest: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub volatile: Option<bool>,
}

impl Check for RecipeDigestFilesItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("digest") { None => out.push(crate::types::check::missing::<Self>("digest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("digest"))), } } }
        if let Some(v) = obj.get("volatile") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("volatile"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "dest" => <String as Check>::describe(rest),
                "digest" => <String as Check>::describe(rest),
                "volatile" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:134 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigest {
    pub ticks: Vec<RecipeDigestTicksItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub login: Option<String>,
    pub files: Vec<RecipeDigestFilesItem>,
}

impl Check for RecipeDigest {
    /// Nested checks: ticks, files.
    fn check(&self) -> Result<(), SchemaError> {
        self.ticks.check().map_err(|e| e.under("ticks"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("ticks") { None => out.push(crate::types::check::missing::<Self>("ticks")), Some(v) => { match crate::types::check::read::<Vec<RecipeDigestTicksItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ticks"))), } } }
        if let Some(v) = obj.get("login") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("login"))), } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<RecipeDigestFilesItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ticks" => <Vec<RecipeDigestTicksItem> as Check>::describe(rest),
                "login" => <Option<String> as Check>::describe(rest),
                "files" => <Vec<RecipeDigestFilesItem> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceProject {
    pub why: String,
}

impl RecipeSourceProject {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "project";
}

impl Check for RecipeSourceProject {
    /// Rules: why: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.why, 1).map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_why(&x) { out.push(e.under("why")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "why" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeSourceProject {
    fn rule_why(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceInstalled {
    pub paths: Vec<String>,
    pub bin: bool,
}

impl RecipeSourceInstalled {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "installed";
}

impl Check for RecipeSourceInstalled {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("paths") { None => out.push(crate::types::check::missing::<Self>("paths")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
        match obj.get("bin") { None => out.push(crate::types::check::missing::<Self>("bin")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bin"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "paths" => <Vec<String> as Check>::describe(rest),
                "bin" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceUsed {
    pub sessions: JsInt,
    pub calls: JsInt,
}

impl RecipeSourceUsed {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "used";
}

impl Check for RecipeSourceUsed {
    /// Rules: sessions: >= 0; calls: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.sessions.0.0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.calls.0.0, 0.0).map_err(|e| e.under("calls"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("sessions") { None => out.push(crate::types::check::missing::<Self>("sessions")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_sessions(&x) { out.push(e.under("sessions")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } } }
        match obj.get("calls") { None => out.push(crate::types::check::missing::<Self>("calls")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_calls(&x) { out.push(e.under("calls")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("calls"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "sessions" => <i64 as Check>::describe(rest),
                "calls" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeSourceUsed {
    fn rule_sessions(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_calls(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourcePopular {
    pub sessions: JsInt,
    pub images: JsInt,
}

impl RecipeSourcePopular {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "popular";
}

impl Check for RecipeSourcePopular {
    /// Rules: sessions: >= 0; images: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.sessions.0.0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.images.0.0, 0.0).map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("sessions") { None => out.push(crate::types::check::missing::<Self>("sessions")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_sessions(&x) { out.push(e.under("sessions")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } } }
        match obj.get("images") { None => out.push(crate::types::check::missing::<Self>("images")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_images(&x) { out.push(e.under("images")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("images"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "sessions" => <i64 as Check>::describe(rest),
                "images" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeSourcePopular {
    fn rule_sessions(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_images(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:161 at a3b870a6e.
/// zod's discriminatedUnion on `kind`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RecipeSource {
    #[serde(rename = "project")]
    Project(RecipeSourceProject),
    #[serde(rename = "installed")]
    Installed(RecipeSourceInstalled),
    #[serde(rename = "used")]
    Used(RecipeSourceUsed),
    #[serde(rename = "popular")]
    Popular(RecipeSourcePopular),
}

impl Check for RecipeSource {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            RecipeSource::Project(v) => v.check(),
            RecipeSource::Installed(v) => v.check(),
            RecipeSource::Used(v) => v.check(),
            RecipeSource::Popular(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("kind") else { return Some(vec![crate::types::check::missing::<Self>("kind")]); };
        match tag.as_str() {
            Some("project") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<RecipeSourceProject>(&serde_json::Value::Object(body))) }
            Some("installed") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<RecipeSourceInstalled>(&serde_json::Value::Object(body))) }
            Some("used") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<RecipeSourceUsed>(&serde_json::Value::Object(body))) }
            Some("popular") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<RecipeSourcePopular>(&serde_json::Value::Object(body))) }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("kind", tag)]),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Word("object")); }
        if path.len() == 1 && path[0].as_str() == Some("kind") { return Some(crate::types::check::Zod::Tags("kind", &["project", "installed", "used", "popular"])); }
        <RecipeSourceProject as Check>::describe(path).or_else(|| <RecipeSourceInstalled as Check>::describe(path)).or_else(|| <RecipeSourceUsed as Check>::describe(path)).or_else(|| <RecipeSourcePopular as Check>::describe(path))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeRowKind {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
}

impl Check for RecipeRowKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["agent", "tool"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:173 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeRow {
    pub id: String,
    pub kind: RecipeRowKind,
    pub on: bool,
    pub source: RecipeSource,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sign_in: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pin: Option<ToolPin>,
}

impl Check for RecipeRow {
    /// Rules: id: at least 1 UTF-16 units; size: >= 0.
    /// Nested checks: source, signIn, pin.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.source.check().map_err(|e| e.under("source"))?;
        if let Some(v) = self.size {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("size"))?;
        }
        self.sign_in.check().map_err(|e| e.under("signIn"))?;
        self.pin.check().map_err(|e| e.under("pin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_id(&x) { out.push(e.under("id")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<RecipeRowKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("on") { None => out.push(crate::types::check::missing::<Self>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<RecipeSource>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size(&x) { out.push(e.under("size")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("signIn") { match crate::types::check::read_present::<LoginChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signIn"))), } }
        if let Some(v) = obj.get("pin") { match crate::types::check::read_present::<ToolPin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pin"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <RecipeRowKind as Check>::describe(rest),
                "on" => <bool as Check>::describe(rest),
                "source" => <RecipeSource as Check>::describe(rest),
                "size" => <Option<i64> as Check>::describe(rest),
                "signIn" => <Option<LoginChoice> as Check>::describe(rest),
                "pin" => <Option<ToolPin> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeRow {
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeHistoryState {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "unreadable")]
    Unreadable,
    #[serde(rename = "no-reader")]
    NoReader,
}

impl Check for RecipeHistoryState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["read", "empty", "unreadable", "no-reader"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:189 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistory {
    pub agent: String,
    pub state: RecipeHistoryState,
    pub sessions: JsInt,
    pub calls: JsInt,
}

impl Check for RecipeHistory {
    /// Rules: agent: at least 1 UTF-16 units; sessions: >= 0; calls: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.agent, 1).map_err(|e| e.under("agent"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        crate::types::check::num_at_least(self.sessions.0.0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.calls.0.0, 0.0).map_err(|e| e.under("calls"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_agent(&x) { out.push(e.under("agent")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<RecipeHistoryState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        match obj.get("sessions") { None => out.push(crate::types::check::missing::<Self>("sessions")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_sessions(&x) { out.push(e.under("sessions")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } } }
        match obj.get("calls") { None => out.push(crate::types::check::missing::<Self>("calls")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_calls(&x) { out.push(e.under("calls")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("calls"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "state" => <RecipeHistoryState as Check>::describe(rest),
                "sessions" => <i64 as Check>::describe(rest),
                "calls" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeHistory {
    fn rule_agent(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_sessions(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_calls(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:200 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeTick {
    #[serde(rename = "used")]
    Used,
    #[serde(rename = "installed")]
    Installed,
    #[serde(rename = "default")]
    Default,
}

impl Check for RecipeTick {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["used", "installed", "default"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:208 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeCustomRow {
    pub kind: CustomLit,
    pub id: String,
    pub name: String,
    pub install: Vec<String>,
    pub check: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub manager: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<JsInt>,
    pub why: String,
}

impl Check for RecipeCustomRow {
    /// Rules: id: at least 1 UTF-16 units; name: at least 1 UTF-16 units; install[]: at least 1 UTF-16 units; install: at least 1 items; check: at least 1 UTF-16 units; manager: at least 1 UTF-16 units; size: >= 0; why: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        for (i, item) in self.install.iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("install"))?;
        }
        crate::types::check::items_at_least(self.install.len(), 1).map_err(|e| e.under("install"))?;
        crate::types::check::text_at_least(&self.check, 1).map_err(|e| e.under("check"))?;
        if let Some(v) = &self.manager {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("manager"))?;
        }
        if let Some(v) = self.size {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("size"))?;
        }
        crate::types::check::text_at_least(&self.why, 1).map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<CustomLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_id(&x) { out.push(e.under("id")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_name(&x) { out.push(e.under("name")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("install") { None => out.push(crate::types::check::missing::<Self>("install")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(x) => { if let Err(e) = Self::rule_install(&x) { out.push(e.under("install")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("install"))), } } }
        match obj.get("check") { None => out.push(crate::types::check::missing::<Self>("check")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_check(&x) { out.push(e.under("check")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } } }
        if let Some(v) = obj.get("manager") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_manager(&x) { out.push(e.under("manager")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("manager"))), } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size(&x) { out.push(e.under("size")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_why(&x) { out.push(e.under("why")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <CustomLit as Check>::describe(rest),
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "install" => <Vec<String> as Check>::describe(rest),
                "check" => <String as Check>::describe(rest),
                "manager" => <Option<String> as Check>::describe(rest),
                "size" => <Option<i64> as Check>::describe(rest),
                "why" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeCustomRow {
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_install(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        for (i, item) in (*x).iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
        }
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
    fn rule_check(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_manager(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
    }
    fn rule_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_why(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:228 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub version: LitV1,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tick: Option<RecipeTick>,
    pub histories: Vec<RecipeHistory>,
    pub rows: Vec<RecipeRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub custom: Option<Vec<RecipeCustomRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub engine: Option<bool>,
}

impl Check for Recipe {
    /// Rules: at: at least 1 UTF-16 units.
    /// Nested checks: tick, histories, rows, custom.
    fn check(&self) -> Result<(), SchemaError> {
        self.version.check().map_err(|e| e.under("version"))?;
        crate::types::check::text_at_least(&self.at, 1).map_err(|e| e.under("at"))?;
        self.tick.check().map_err(|e| e.under("tick"))?;
        self.histories.check().map_err(|e| e.under("histories"))?;
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.custom.check().map_err(|e| e.under("custom"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<LitV1>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_at(&x) { out.push(e.under("at")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        if let Some(v) = obj.get("tick") { match crate::types::check::read_present::<RecipeTick>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tick"))), } }
        match obj.get("histories") { None => out.push(crate::types::check::missing::<Self>("histories")), Some(v) => { match crate::types::check::read::<Vec<RecipeHistory>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("histories"))), } } }
        match obj.get("rows") { None => out.push(crate::types::check::missing::<Self>("rows")), Some(v) => { match crate::types::check::read::<Vec<RecipeRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } } }
        if let Some(v) = obj.get("custom") { match crate::types::check::read_present::<Vec<RecipeCustomRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("custom"))), } }
        if let Some(v) = obj.get("engine") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("engine"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "version" => <LitV1 as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "tick" => <Option<RecipeTick> as Check>::describe(rest),
                "histories" => <Vec<RecipeHistory> as Check>::describe(rest),
                "rows" => <Vec<RecipeRow> as Check>::describe(rest),
                "custom" => <Option<Vec<RecipeCustomRow>> as Check>::describe(rest),
                "engine" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl Recipe {
    fn rule_at(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewSize {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
}

impl Check for GoldenBuilderViewSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <JsNumber as Check>::describe(rest),
                "memMb" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewScreen {
    pub stream_url: String,
}

impl Check for GoldenBuilderViewScreen {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("streamUrl") { None => out.push(crate::types::check::missing::<Self>("streamUrl")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("streamUrl"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "streamUrl" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewHeldBy {
    pub host: String,
    pub pid: JsNumber,
    pub heartbeat: String,
}

impl Check for GoldenBuilderViewHeldBy {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("host") { None => out.push(crate::types::check::missing::<Self>("host")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("host"))), } } }
        match obj.get("pid") { None => out.push(crate::types::check::missing::<Self>("pid")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
        match obj.get("heartbeat") { None => out.push(crate::types::check::missing::<Self>("heartbeat")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("heartbeat"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "host" => <String as Check>::describe(rest),
                "pid" => <JsNumber as Check>::describe(rest),
                "heartbeat" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewSealed {
    pub at: String,
    pub version: JsNumber,
}

impl Check for GoldenBuilderViewSealed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <String as Check>::describe(rest),
                "version" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:260 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderView {
    pub id: String,
    pub name: String,
    pub kind: MachineKind,
    pub created_at: String,
    pub place_id: String,
    pub place_name: String,
    pub size: GoldenBuilderViewSize,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub screen: Option<GoldenBuilderViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sealable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<RecipeDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub foreign_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub held_by: Option<GoldenBuilderViewHeldBy>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub building: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sealed: Option<GoldenBuilderViewSealed>,
}

impl Check for GoldenBuilderView {
    /// Nested checks: kind, size, screen, recipe, heldBy, sealed.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.screen.check().map_err(|e| e.under("screen"))?;
        self.recipe.check().map_err(|e| e.under("recipe"))?;
        self.held_by.check().map_err(|e| e.under("heldBy"))?;
        self.sealed.check().map_err(|e| e.under("sealed"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<MachineKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("placeName") { None => out.push(crate::types::check::missing::<Self>("placeName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeName"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<GoldenBuilderViewSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        if let Some(v) = obj.get("rateUsdPerHour") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } }
        if let Some(v) = obj.get("screen") { match crate::types::check::read_present::<GoldenBuilderViewScreen>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("screen"))), } }
        if let Some(v) = obj.get("sealable") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sealable"))), } }
        if let Some(v) = obj.get("recipeHash") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipeHash"))), } }
        if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<RecipeDigest>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
        if let Some(v) = obj.get("foreignOwner") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("foreignOwner"))), } }
        if let Some(v) = obj.get("heldBy") { match crate::types::check::read_present::<GoldenBuilderViewHeldBy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("heldBy"))), } }
        if let Some(v) = obj.get("building") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("building"))), } }
        if let Some(v) = obj.get("sealed") { match crate::types::check::read_present::<GoldenBuilderViewSealed>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sealed"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "kind" => <MachineKind as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "placeId" => <String as Check>::describe(rest),
                "placeName" => <String as Check>::describe(rest),
                "size" => <GoldenBuilderViewSize as Check>::describe(rest),
                "rateUsdPerHour" => <Option<JsNumber> as Check>::describe(rest),
                "screen" => <Option<GoldenBuilderViewScreen> as Check>::describe(rest),
                "sealable" => <Option<bool> as Check>::describe(rest),
                "recipeHash" => <Option<String> as Check>::describe(rest),
                "recipe" => <Option<RecipeDigest> as Check>::describe(rest),
                "foreignOwner" => <Option<String> as Check>::describe(rest),
                "heldBy" => <Option<GoldenBuilderViewHeldBy> as Check>::describe(rest),
                "building" => <Option<bool> as Check>::describe(rest),
                "sealed" => <Option<GoldenBuilderViewSealed> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:293 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoldenStage {
    #[serde(rename = "creating")]
    Creating,
    #[serde(rename = "deploying-daemon")]
    DeployingDaemon,
    #[serde(rename = "applying-setup")]
    ApplyingSetup,
    #[serde(rename = "uploading-files")]
    UploadingFiles,
    #[serde(rename = "installing-harness")]
    InstallingHarness,
    #[serde(rename = "installing-tools")]
    InstallingTools,
    #[serde(rename = "installing-mcp")]
    InstallingMcp,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "snapshotting")]
    Snapshotting,
    #[serde(rename = "promoting")]
    Promoting,
    #[serde(rename = "smoke-forking")]
    SmokeForking,
    #[serde(rename = "sealed")]
    Sealed,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for GoldenStage {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["creating", "deploying-daemon", "applying-setup", "uploading-files", "installing-harness", "installing-tools", "installing-mcp", "ready", "snapshotting", "promoting", "smoke-forking", "sealed", "failed"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:312 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenStep {
    pub label: String,
    pub command: String,
}

impl Check for GoldenStep {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("label") { None => out.push(crate::types::check::missing::<Self>("label")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } } }
        match obj.get("command") { None => out.push(crate::types::check::missing::<Self>("command")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "label" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:317 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenStageEvent {
    pub name: String,
    pub stage: GoldenStage,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub step: Option<GoldenStep>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub left: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
}

impl GoldenStageEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "golden.stage";
}

impl Check for GoldenStageEvent {
    /// Nested checks: stage, step.
    fn check(&self) -> Result<(), SchemaError> {
        self.stage.check().map_err(|e| e.under("stage"))?;
        self.step.check().map_err(|e| e.under("step"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("stage") { None => out.push(crate::types::check::missing::<Self>("stage")), Some(v) => { match crate::types::check::read::<GoldenStage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stage"))), } } }
        if let Some(v) = obj.get("detail") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("detail"))), } }
        if let Some(v) = obj.get("step") { match crate::types::check::read_present::<GoldenStep>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("step"))), } }
        if let Some(v) = obj.get("left") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "stage" => <GoldenStage as Check>::describe(rest),
                "detail" => <Option<String> as Check>::describe(rest),
                "step" => <Option<GoldenStep> as Check>::describe(rest),
                "left" => <Option<Vec<String>> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:334 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedVault {
    pub sha256: String,
    pub bytes: JsInt,
    pub paths: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub held: Option<Vec<String>>,
    pub taken_at: String,
}

impl Check for SealedVault {
    /// Rules: sha256: exactly 64 UTF-16 units; bytes: >= 0; paths: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_exactly(&self.sha256, 64).map_err(|e| e.under("sha256"))?;
        crate::types::check::num_at_least(self.bytes.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::num_at_least(self.paths.0.0, 0.0).map_err(|e| e.under("paths"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("sha256") { None => out.push(crate::types::check::missing::<Self>("sha256")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_sha256(&x) { out.push(e.under("sha256")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sha256"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("paths") { None => out.push(crate::types::check::missing::<Self>("paths")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_paths(&x) { out.push(e.under("paths")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
        if let Some(v) = obj.get("held") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("held"))), } }
        match obj.get("takenAt") { None => out.push(crate::types::check::missing::<Self>("takenAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("takenAt"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "sha256" => <String as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "paths" => <i64 as Check>::describe(rest),
                "held" => <Option<Vec<String>> as Check>::describe(rest),
                "takenAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SealedVault {
    fn rule_sha256(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_exactly(x, 64)?;
        Ok(())
    }
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_paths(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:349 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedPin {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub arch: Option<ToolPinArch>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub latest: Option<True>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<String>,
}

impl Check for SealedPin {
    /// Rules: tag: at least 1 UTF-16 units; sha256: at least 1 UTF-16 units; id: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.tag, 1).map_err(|e| e.under("tag"))?;
        if let Some(v) = &self.sha256 {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("sha256"))?;
        }
        self.arch.check().map_err(|e| e.under("arch"))?;
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("tag") { None => out.push(crate::types::check::missing::<Self>("tag")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_tag(&x) { out.push(e.under("tag")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("tag"))), } } }
        if let Some(v) = obj.get("sha256") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_sha256(&x) { out.push(e.under("sha256")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sha256"))), } }
        if let Some(v) = obj.get("arch") { match crate::types::check::read_present::<ToolPinArch>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("arch"))), } }
        if let Some(v) = obj.get("latest") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("latest"))), } }
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_id(&x) { out.push(e.under("id")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("road") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "tag" => <String as Check>::describe(rest),
                "sha256" => <Option<String> as Check>::describe(rest),
                "arch" => <Option<ToolPinArch> as Check>::describe(rest),
                "latest" => None,
                "id" => <String as Check>::describe(rest),
                "road" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SealedPin {
    fn rule_tag(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_sha256(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
    }
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:362 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImage {
    pub name: String,
    pub version: JsInt,
    pub hash: String,
    pub recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<Recipe>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pins: Option<Vec<SealedPin>>,
    pub logins: Vec<GoldenLogin>,
    pub sealed_at: String,
    pub sealed_from: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault: Option<SealedVault>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub used_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub no_secrets: Option<bool>,
}

impl Check for SealedImage {
    /// Rules: version: > 0; hash: exactly 64 UTF-16 units; usedBytes: >= 0.
    /// Nested checks: recipe, pins, logins, vault.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.version.0.0, 0.0).map_err(|e| e.under("version"))?;
        crate::types::check::text_exactly(&self.hash, 64).map_err(|e| e.under("hash"))?;
        self.recipe.check().map_err(|e| e.under("recipe"))?;
        self.pins.check().map_err(|e| e.under("pins"))?;
        self.logins.check().map_err(|e| e.under("logins"))?;
        self.vault.check().map_err(|e| e.under("vault"))?;
        if let Some(v) = self.used_bytes {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("usedBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_version(&x) { out.push(e.under("version")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        match obj.get("hash") { None => out.push(crate::types::check::missing::<Self>("hash")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_hash(&x) { out.push(e.under("hash")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("hash"))), } } }
        match obj.get("recipeHash") { None => out.push(crate::types::check::missing::<Self>("recipeHash")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipeHash"))), } } }
        if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<Recipe>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
        if let Some(v) = obj.get("pins") { match crate::types::check::read_present::<Vec<SealedPin>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pins"))), } }
        match obj.get("logins") { None => out.push(crate::types::check::missing::<Self>("logins")), Some(v) => { match crate::types::check::read::<Vec<GoldenLogin>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("logins"))), } } }
        match obj.get("sealedAt") { None => out.push(crate::types::check::missing::<Self>("sealedAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sealedAt"))), } } }
        match obj.get("sealedFrom") { None => out.push(crate::types::check::missing::<Self>("sealedFrom")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sealedFrom"))), } } }
        if let Some(v) = obj.get("vault") { match crate::types::check::read_present::<SealedVault>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vault"))), } }
        if let Some(v) = obj.get("usedBytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_used_bytes(&x) { out.push(e.under("usedBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("usedBytes"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("noSecrets") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("noSecrets"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "version" => <i64 as Check>::describe(rest),
                "hash" => <String as Check>::describe(rest),
                "recipeHash" => <String as Check>::describe(rest),
                "recipe" => <Option<Recipe> as Check>::describe(rest),
                "pins" => <Option<Vec<SealedPin>> as Check>::describe(rest),
                "logins" => <Vec<GoldenLogin> as Check>::describe(rest),
                "sealedAt" => <String as Check>::describe(rest),
                "sealedFrom" => <String as Check>::describe(rest),
                "vault" => <Option<SealedVault> as Check>::describe(rest),
                "usedBytes" => <Option<i64> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "noSecrets" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SealedImage {
    fn rule_version(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_hash(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_exactly(x, 64)?;
        Ok(())
    }
    fn rule_used_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:393 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageCopy {
    pub place: String,
    pub version: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hash: Option<String>,
    pub snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub template_id: Option<String>,
    pub built_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size_bytes: Option<JsInt>,
}

impl Check for SealedImageCopy {
    /// Rules: version: > 0; hash: exactly 64 UTF-16 units; sizeBytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.version.0.0, 0.0).map_err(|e| e.under("version"))?;
        if let Some(v) = &self.hash {
            crate::types::check::text_exactly(v, 64).map_err(|e| e.under("hash"))?;
        }
        if let Some(v) = self.size_bytes {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("sizeBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        match obj.get("version") { None => out.push(crate::types::check::missing::<Self>("version")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_version(&x) { out.push(e.under("version")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
        if let Some(v) = obj.get("hash") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_hash(&x) { out.push(e.under("hash")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("hash"))), } }
        match obj.get("snapshotId") { None => out.push(crate::types::check::missing::<Self>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
        if let Some(v) = obj.get("templateId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("templateId"))), } }
        match obj.get("builtAt") { None => out.push(crate::types::check::missing::<Self>("builtAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("builtAt"))), } } }
        if let Some(v) = obj.get("sizeBytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size_bytes(&x) { out.push(e.under("sizeBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeBytes"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <String as Check>::describe(rest),
                "version" => <i64 as Check>::describe(rest),
                "hash" => <Option<String> as Check>::describe(rest),
                "snapshotId" => <String as Check>::describe(rest),
                "templateId" => <Option<String> as Check>::describe(rest),
                "builtAt" => <String as Check>::describe(rest),
                "sizeBytes" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SealedImageCopy {
    fn rule_version(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_hash(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_exactly(v, 64)?;
        }
        Ok(())
    }
    fn rule_size_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:411 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageBuilt {
    pub copy: SealedImageCopy,
    pub built: bool,
}

impl Check for SealedImageBuilt {
    /// Nested checks: copy.
    fn check(&self) -> Result<(), SchemaError> {
        self.copy.check().map_err(|e| e.under("copy"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("copy") { None => out.push(crate::types::check::missing::<Self>("copy")), Some(v) => { match crate::types::check::read::<SealedImageCopy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copy"))), } } }
        match obj.get("built") { None => out.push(crate::types::check::missing::<Self>("built")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("built"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "copy" => <SealedImageCopy as Check>::describe(rest),
                "built" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:415 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageExport {
    pub path: String,
    pub bytes: JsInt,
    pub hash: String,
}

impl Check for SealedImageExport {
    /// Rules: bytes: >= 0; hash: exactly 64 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::text_exactly(&self.hash, 64).map_err(|e| e.under("hash"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("hash") { None => out.push(crate::types::check::missing::<Self>("hash")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_hash(&x) { out.push(e.under("hash")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("hash"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "hash" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SealedImageExport {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_hash(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_exactly(x, 64)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SealedVaultHeaderTo {
    #[serde(rename = "passphrase")]
    Passphrase,
    #[serde(rename = "key")]
    Key,
}

impl Check for SealedVaultHeaderTo {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["passphrase", "key"]))
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:424 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedVaultHeader {
    pub format: WspVault1Lit,
    pub cipher: Aes256GcmLit,
    pub to: SealedVaultHeaderTo,
    pub salt: String,
    pub nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ephemeral: Option<String>,
    pub image: SealedImage,
}

impl Check for SealedVaultHeader {
    /// Nested checks: image.
    fn check(&self) -> Result<(), SchemaError> {
        self.format.check().map_err(|e| e.under("format"))?;
        self.cipher.check().map_err(|e| e.under("cipher"))?;
        self.to.check().map_err(|e| e.under("to"))?;
        self.image.check().map_err(|e| e.under("image"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("format") { None => out.push(crate::types::check::missing::<Self>("format")), Some(v) => { match crate::types::check::read::<WspVault1Lit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("format"))), } } }
        match obj.get("cipher") { None => out.push(crate::types::check::missing::<Self>("cipher")), Some(v) => { match crate::types::check::read::<Aes256GcmLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cipher"))), } } }
        match obj.get("to") { None => out.push(crate::types::check::missing::<Self>("to")), Some(v) => { match crate::types::check::read::<SealedVaultHeaderTo>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("to"))), } } }
        match obj.get("salt") { None => out.push(crate::types::check::missing::<Self>("salt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("salt"))), } } }
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        if let Some(v) = obj.get("ephemeral") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } }
        match obj.get("image") { None => out.push(crate::types::check::missing::<Self>("image")), Some(v) => { match crate::types::check::read::<SealedImage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("image"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "format" => <WspVault1Lit as Check>::describe(rest),
                "cipher" => <Aes256GcmLit as Check>::describe(rest),
                "to" => <SealedVaultHeaderTo as Check>::describe(rest),
                "salt" => <String as Check>::describe(rest),
                "nonce" => <String as Check>::describe(rest),
                "ephemeral" => <Option<String> as Check>::describe(rest),
                "image" => <SealedImage as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// ---- Hand-written below: the functions of the unit.

// views/golden-image.ts:97 at a3b870a6e
// export const onNpmBin = (npmBin: string | undefined, path: string): string => (npmBin === undefined ? path : `${AGENTS_BIN}:${npmBin}:${path}`);
/// `path` alone without an npm bin; else `AGENTS_BIN:<npm bin>:<path>`.
pub fn on_npm_bin(npm_bin: Option<&str>, path: &str) -> String {
    match npm_bin {
        None => path.to_string(),
        Some(bin) => format!("{AGENTS_BIN}:{bin}:{path}"),
    }
}

// views/golden-image.ts:103 at a3b870a6e
// export function goldenHead(manifest: GoldenManifest | undefined): GoldenVersion | undefined {
//   return manifest?.versions.find(v => v.version === manifest.head);
// }
/// The version whose number equals the manifest's head; None without a manifest or with no such version.
pub fn golden_head(manifest: Option<&GoldenManifest>) -> Option<&GoldenVersion> {
    let manifest = manifest?;
    manifest.versions.iter().find(|v| v.version.0 == manifest.head.0)
}

// views/golden-image.ts:110 at a3b870a6e
// export const holdsNothing = (golden: GoldenManifest | undefined, workspaces: readonly unknown[]): boolean => goldenHead(golden) === undefined && workspaces.length === 0;
pub fn holds_nothing(golden: Option<&GoldenManifest>, workspaces: usize) -> bool {
    golden_head(golden).is_none() && workspaces == 0
}

/// What a fork boots from: the version's template where it has one, else its snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSpec {
    Template(String),
    FromSnapshot(String),
}

/// `golden_image`'s answer: the spec, and `["volatile"]` for a snapshot, `[]` for a template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenImage {
    pub spec: ImageSpec,
    pub marks: Vec<&'static str>,
}

// views/golden-image.ts:116 at a3b870a6e
// export function goldenImage(v: Pick<GoldenVersion, "snapshotId" | "templateId">): { spec: { template: string } | { fromSnapshot: string }; marks: readonly "volatile"[] } {
//   return v.templateId !== undefined ? { spec: { template: v.templateId }, marks: [] } : { spec: { fromSnapshot: v.snapshotId }, marks: ["volatile"] };
// }
pub fn golden_image(v: &GoldenVersion) -> GoldenImage {
    match &v.template_id {
        Some(template) => GoldenImage { spec: ImageSpec::Template(template.clone()), marks: Vec::new() },
        None => GoldenImage { spec: ImageSpec::FromSnapshot(v.snapshot_id.clone()), marks: vec!["volatile"] },
    }
}

// views/golden-image.ts:245 at a3b870a6e
// export function customRows(recipe: Pick<Recipe, "custom">): readonly RecipeCustomRow[] {
//   return recipe.custom ?? [];
// }
pub fn custom_rows(recipe: &Recipe) -> Vec<RecipeCustomRow> {
    recipe.custom.clone().unwrap_or_default()
}

// views/golden-image.ts:250 at a3b870a6e
// export function commandCheck(bin: string): string {
//   return `command -v ${shellQuote(bin)}`;
// }
/// `command -v <bin quoted by crate::types::shell_quote::shell_quote>`.
pub fn command_check(bin: &str) -> String {
    format!("command -v {}", crate::types::shell_quote::shell_quote(bin))
}

// views/golden-image.ts:355 at a3b870a6e
// export function recipePins(digest: Pick<RecipeDigest, "ticks">, key: (id: string) => string = id => id): SealedPin[] {
//   return digest.ticks
//     .flatMap((t): SealedPin[] => (t.pin === undefined ? [] : [{ id: key(t.id), ...t.pin, ...(t.road !== undefined ? { road: t.road } : {}) }]))
//     .sort((a, b) => a.id.localeCompare(b.id));
// }
/// One pin per tick that has one, its id through `key` (the identity where the TS caller passes none), then the pin's
/// own keys, then `road` where the tick has one; sorted by id as `String.prototype.localeCompare` orders them
/// (`crate::text::collate`).
pub fn recipe_pins(digest: &RecipeDigest, key: Option<&dyn Fn(&str) -> String>) -> Vec<SealedPin> {
    let mut pins: Vec<SealedPin> = digest
        .ticks
        .iter()
        .filter_map(|tick| {
            let pin = tick.pin.as_ref()?;
            let id = key.map_or_else(|| tick.id.clone(), |key| key(&tick.id));
            Some(SealedPin {
                tag: pin.tag.clone(),
                sha256: pin.sha256.clone(),
                arch: pin.arch.clone(),
                latest: pin.latest,
                id,
                road: tick.road.clone(),
            })
        })
        .collect();
    pins.sort_by(|a, b| crate::text::collate(&a.id, &b.id));
    pins
}

// views/golden-image.ts:442 at a3b870a6e (private)
// const AGENTS_PREFIX = "agents/";
const AGENTS_PREFIX: &str = "agents/";

// views/golden-image.ts:445 at a3b870a6e
// export const agentOfRow = (e: { id: string }): string | undefined => (e.id.startsWith(AGENTS_PREFIX) && !e.id.startsWith(MCP_ID_PREFIX) ? e.id.slice(AGENTS_PREFIX.length) : undefined);
/// The agent an `agents/<id>` row is, never an `agents/mcp/` one.
pub fn agent_of_row(id: &str) -> Option<String> {
    if id.starts_with(MCP_ID_PREFIX) {
        return None;
    }
    id.strip_prefix(AGENTS_PREFIX).map(String::from)
}

// views/golden-image.ts:448 at a3b870a6e
// export const toolRowPrefix = (manager: string): string => `tools/${manager}/`;
pub fn tool_row_prefix(manager: &str) -> String {
    format!("tools/{manager}/")
}

// views/golden-image.ts:451 at a3b870a6e
// export const toolRowId = (manager: string, pkg: string): string => `${toolRowPrefix(manager)}${pkg}`;
pub fn tool_row_id(manager: &str, pkg: &str) -> String {
    format!("{}{pkg}", tool_row_prefix(manager))
}

// views/golden-image.ts:456 at a3b870a6e
// export const packageOf = (e: { id: string }): string => e.id.split("/").slice(2).join("/");
/// Everything after the second `/` of the id, slashes kept; "" where there is none.
pub fn package_of(id: &str) -> String {
    id.split('/').skip(2).collect::<Vec<_>>().join("/")
}
