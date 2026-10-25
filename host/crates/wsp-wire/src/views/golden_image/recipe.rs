// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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
        crate::types::keys::req_key::<LitV1>(obj, "version", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "at", &mut out, Self::rule_at);
        crate::types::keys::opt_key::<RecipeTick>(obj, "tick", &mut out);
        crate::types::keys::req_key::<Vec<RecipeHistory>>(obj, "histories", &mut out);
        crate::types::keys::req_key::<Vec<RecipeRow>>(obj, "rows", &mut out);
        crate::types::keys::opt_key::<Vec<RecipeCustomRow>>(obj, "custom", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "engine", &mut out);
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
        crate::types::keys::req_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "memMb", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "streamUrl", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "host", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "pid", &mut out);
        crate::types::keys::req_key::<String>(obj, "heartbeat", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "version", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<MachineKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "placeName", &mut out);
        crate::types::keys::req_key::<GoldenBuilderViewSize>(obj, "size", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::opt_key::<GoldenBuilderViewScreen>(obj, "screen", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "sealable", &mut out);
        crate::types::keys::opt_key::<String>(obj, "recipeHash", &mut out);
        crate::types::keys::opt_key::<RecipeDigest>(obj, "recipe", &mut out);
        crate::types::keys::opt_key::<String>(obj, "foreignOwner", &mut out);
        crate::types::keys::opt_key::<GoldenBuilderViewHeldBy>(obj, "heldBy", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "building", &mut out);
        crate::types::keys::opt_key::<GoldenBuilderViewSealed>(obj, "sealed", &mut out);
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "creating",
                "deploying-daemon",
                "applying-setup",
                "uploading-files",
                "installing-harness",
                "installing-tools",
                "installing-mcp",
                "ready",
                "snapshotting",
                "promoting",
                "smoke-forking",
                "sealed",
                "failed",
            ]),
        )
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
        crate::types::keys::req_key::<String>(obj, "label", &mut out);
        crate::types::keys::req_key::<String>(obj, "command", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<GoldenStage>(obj, "stage", &mut out);
        crate::types::keys::opt_key::<String>(obj, "detail", &mut out);
        crate::types::keys::opt_key::<GoldenStep>(obj, "step", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "left", &mut out);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
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
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::num_at_least(self.paths.0 .0, 0.0).map_err(|e| e.under("paths"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "sha256", &mut out, Self::rule_sha256);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::req_key_with::<JsInt>(obj, "paths", &mut out, Self::rule_paths);
        crate::types::keys::opt_key::<Vec<String>>(obj, "held", &mut out);
        crate::types::keys::req_key::<String>(obj, "takenAt", &mut out);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_paths(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        crate::types::keys::req_key_with::<String>(obj, "tag", &mut out, Self::rule_tag);
        crate::types::keys::opt_key_with::<String>(obj, "sha256", &mut out, Self::rule_sha256);
        crate::types::keys::opt_key::<ToolPinArch>(obj, "arch", &mut out);
        crate::types::keys::opt_key::<True>(obj, "latest", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::opt_key::<String>(obj, "road", &mut out);
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
        crate::types::check::num_above(self.version.0 .0, 0.0).map_err(|e| e.under("version"))?;
        crate::types::check::text_exactly(&self.hash, 64).map_err(|e| e.under("hash"))?;
        self.recipe.check().map_err(|e| e.under("recipe"))?;
        self.pins.check().map_err(|e| e.under("pins"))?;
        self.logins.check().map_err(|e| e.under("logins"))?;
        self.vault.check().map_err(|e| e.under("vault"))?;
        if let Some(v) = self.used_bytes {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("usedBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "version", &mut out, Self::rule_version);
        crate::types::keys::req_key_with::<String>(obj, "hash", &mut out, Self::rule_hash);
        crate::types::keys::req_key::<String>(obj, "recipeHash", &mut out);
        crate::types::keys::opt_key::<Recipe>(obj, "recipe", &mut out);
        crate::types::keys::opt_key::<Vec<SealedPin>>(obj, "pins", &mut out);
        crate::types::keys::req_key::<Vec<GoldenLogin>>(obj, "logins", &mut out);
        crate::types::keys::req_key::<String>(obj, "sealedAt", &mut out);
        crate::types::keys::req_key::<String>(obj, "sealedFrom", &mut out);
        crate::types::keys::opt_key::<SealedVault>(obj, "vault", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "usedBytes", &mut out, Self::rule_used_bytes);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "noSecrets", &mut out);
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
        crate::types::check::num_above(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_hash(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_exactly(x, 64)?;
        Ok(())
    }
    fn rule_used_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::check::num_above(self.version.0 .0, 0.0).map_err(|e| e.under("version"))?;
        if let Some(v) = &self.hash {
            crate::types::check::text_exactly(v, 64).map_err(|e| e.under("hash"))?;
        }
        if let Some(v) = self.size_bytes {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("sizeBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "place", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "version", &mut out, Self::rule_version);
        crate::types::keys::opt_key_with::<String>(obj, "hash", &mut out, Self::rule_hash);
        crate::types::keys::req_key::<String>(obj, "snapshotId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "templateId", &mut out);
        crate::types::keys::req_key::<String>(obj, "builtAt", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "sizeBytes", &mut out, Self::rule_size_bytes);
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
        crate::types::check::num_above(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_hash(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_exactly(v, 64)?;
        }
        Ok(())
    }
    fn rule_size_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<SealedImageCopy>(obj, "copy", &mut out);
        crate::types::keys::req_key::<bool>(obj, "built", &mut out);
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
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::text_exactly(&self.hash, 64).map_err(|e| e.under("hash"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::req_key_with::<String>(obj, "hash", &mut out, Self::rule_hash);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<WspVault1Lit>(obj, "format", &mut out);
        crate::types::keys::req_key::<Aes256GcmLit>(obj, "cipher", &mut out);
        crate::types::keys::req_key::<SealedVaultHeaderTo>(obj, "to", &mut out);
        crate::types::keys::req_key::<String>(obj, "salt", &mut out);
        crate::types::keys::req_key::<String>(obj, "nonce", &mut out);
        crate::types::keys::opt_key::<String>(obj, "ephemeral", &mut out);
        crate::types::keys::req_key::<SealedImage>(obj, "image", &mut out);
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

/// A pin as `recipePins` builds it, `{ id, ...pin, road }`: the id first, where a parsed `SealedPin` has it after the
/// pin's keys.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipePin {
    pub id: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<ToolPinArch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<True>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub road: Option<String>,
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
pub fn recipe_pins(digest: &RecipeDigest, key: Option<&dyn Fn(&str) -> String>) -> Vec<RecipePin> {
    let mut pins: Vec<RecipePin> = digest
        .ticks
        .iter()
        .filter_map(|tick| {
            let pin = tick.pin.as_ref()?;
            let id = key.map_or_else(|| tick.id.clone(), |key| key(&tick.id));
            Some(RecipePin {
                id,
                tag: pin.tag.clone(),
                sha256: pin.sha256.clone(),
                arch: pin.arch,
                latest: pin.latest,
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
