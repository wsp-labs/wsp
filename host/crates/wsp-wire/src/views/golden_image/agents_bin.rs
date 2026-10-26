// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

literal!(CustomLit, "custom");

literal!(LitV1, 1);

literal!(WspVault1Lit, "wsp-vault-1");

literal!(Aes256GcmLit, "aes-256-gcm");

/// TS: packages/protocol/src/views/golden-image.ts:93 at a3b870a6e.
pub const AGENTS_BIN: &str = "/opt/wsp-agents/bin";

/// TS: packages/protocol/src/views/golden-image.ts:438 at a3b870a6e.
pub const ALREADY_APPLIED: &str = "already applied";

/// TS: packages/protocol/src/views/golden-image.ts:440 at a3b870a6e.
pub const MCP_ID_PREFIX: &str = "agents/mcp/";

/// TS: packages/protocol/src/views/golden-image.ts:453 at a3b870a6e.
pub const BREW_ID_PREFIX: &str = "tools/brew/";

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
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<LoginState>(obj, "state", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<GoldenMissingToolOutcome>(obj, "outcome", &mut out);
        crate::types::keys::req_key::<String>(obj, "note", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "note", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "version", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "cmd", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "exitCode", &mut out);
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("usedBytes"))?;
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
        crate::types::keys::req_key::<JsNumber>(obj, "version", &mut out);
        crate::types::keys::req_key::<String>(obj, "snapshotId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "templateId", &mut out);
        crate::types::keys::req_key::<String>(obj, "baseTemplate", &mut out);
        crate::types::keys::opt_key::<MachineKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "setupSha", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<GoldenVersionSmoke>(obj, "smoke", &mut out);
        crate::types::keys::opt_key::<WorkspaceSize>(obj, "size", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "browserShim", &mut out);
        crate::types::keys::opt_key::<String>(obj, "npmBin", &mut out);
        crate::types::keys::opt_key::<Vec<GoldenLogin>>(obj, "logins", &mut out);
        crate::types::keys::opt_key::<Vec<GoldenMissingTool>>(obj, "missingTools", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "silenced", &mut out);
        crate::types::keys::opt_key::<String>(obj, "shellNoise", &mut out);
        crate::types::keys::opt_key::<Vec<GoldenLeftBehind>>(obj, "leftBehind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentSnapshotId", &mut out);
        crate::types::keys::opt_key::<Vec<GoldenBaseTool>>(obj, "base", &mut out);
        crate::types::keys::opt_key::<Vec<GoldenRetired>>(obj, "retired", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "usedBytes", &mut out, Self::rule_used_bytes);
        crate::types::keys::opt_key_with::<String>(obj, "imageHash", &mut out, Self::rule_image_hash);
        crate::types::keys::opt_key::<bool>(obj, "noSecrets", &mut out);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<JsNumber>(obj, "head", &mut out);
        crate::types::keys::req_key::<Vec<GoldenVersion>>(obj, "versions", &mut out);
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
        crate::types::keys::req_key_with::<String>(obj, "tag", &mut out, Self::rule_tag);
        crate::types::keys::opt_key_with::<String>(obj, "sha256", &mut out, Self::rule_sha256);
        crate::types::keys::opt_key::<ToolPinArch>(obj, "arch", &mut out);
        crate::types::keys::opt_key::<True>(obj, "latest", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key::<LoginChoice>(obj, "choice", &mut out);
        crate::types::keys::opt_key::<String>(obj, "version", &mut out);
        crate::types::keys::opt_key::<String>(obj, "road", &mut out);
        crate::types::keys::opt_key::<String>(obj, "installer", &mut out);
        crate::types::keys::opt_key::<ToolPin>(obj, "pin", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "dest", &mut out);
        crate::types::keys::req_key::<String>(obj, "digest", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "volatile", &mut out);
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
        crate::types::keys::req_key::<Vec<RecipeDigestTicksItem>>(obj, "ticks", &mut out);
        crate::types::keys::opt_key::<String>(obj, "login", &mut out);
        crate::types::keys::req_key::<Vec<RecipeDigestFilesItem>>(obj, "files", &mut out);
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
        crate::types::keys::req_key_with::<String>(obj, "why", &mut out, Self::rule_why);
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
        crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
        crate::types::keys::req_key::<bool>(obj, "bin", &mut out);
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
        crate::types::check::num_at_least(self.sessions.0 .0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.calls.0 .0, 0.0).map_err(|e| e.under("calls"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "sessions", &mut out, Self::rule_sessions);
        crate::types::keys::req_key_with::<JsInt>(obj, "calls", &mut out, Self::rule_calls);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_calls(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        crate::types::check::num_at_least(self.sessions.0 .0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.images.0 .0, 0.0).map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "sessions", &mut out, Self::rule_sessions);
        crate::types::keys::req_key_with::<JsInt>(obj, "images", &mut out, Self::rule_images);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_images(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        let Some(tag) = obj.get("kind") else {
            return Some(vec![crate::types::check::missing::<Self>("kind")]);
        };
        match tag.as_str() {
            Some("project") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<RecipeSourceProject>(&serde_json::Value::Object(body)))
            }
            Some("installed") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<RecipeSourceInstalled>(&serde_json::Value::Object(body)))
            }
            Some("used") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<RecipeSourceUsed>(&serde_json::Value::Object(body)))
            }
            Some("popular") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<RecipeSourcePopular>(&serde_json::Value::Object(body)))
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("kind", tag)]),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Word("object"));
        }
        if path.len() == 1 && path[0].as_str() == Some("kind") {
            return Some(crate::types::check::Zod::Tags("kind", &["project", "installed", "used", "popular"]));
        }
        <RecipeSourceProject as Check>::describe(path)
            .or_else(|| <RecipeSourceInstalled as Check>::describe(path))
            .or_else(|| <RecipeSourceUsed as Check>::describe(path))
            .or_else(|| <RecipeSourcePopular as Check>::describe(path))
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("size"))?;
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
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::req_key::<RecipeRowKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<bool>(obj, "on", &mut out);
        crate::types::keys::req_key::<RecipeSource>(obj, "source", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "size", &mut out, Self::rule_size);
        crate::types::keys::opt_key::<LoginChoice>(obj, "signIn", &mut out);
        crate::types::keys::opt_key::<ToolPin>(obj, "pin", &mut out);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::check::num_at_least(self.sessions.0 .0, 0.0).map_err(|e| e.under("sessions"))?;
        crate::types::check::num_at_least(self.calls.0 .0, 0.0).map_err(|e| e.under("calls"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "agent", &mut out, Self::rule_agent);
        crate::types::keys::req_key::<RecipeHistoryState>(obj, "state", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "sessions", &mut out, Self::rule_sessions);
        crate::types::keys::req_key_with::<JsInt>(obj, "calls", &mut out, Self::rule_calls);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_calls(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("size"))?;
        }
        crate::types::check::text_at_least(&self.why, 1).map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<CustomLit>(obj, "kind", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "install", &mut out, Self::rule_install);
        crate::types::keys::req_key_with::<String>(obj, "check", &mut out, Self::rule_check);
        crate::types::keys::opt_key_with::<String>(obj, "manager", &mut out, Self::rule_manager);
        crate::types::keys::opt_key_with::<JsInt>(obj, "size", &mut out, Self::rule_size);
        crate::types::keys::req_key_with::<String>(obj, "why", &mut out, Self::rule_why);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_why(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}
