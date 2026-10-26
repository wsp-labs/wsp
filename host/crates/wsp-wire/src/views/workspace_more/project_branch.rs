// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/workspace.ts:250 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBranch {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub branch: Option<String>,
    pub folder: bool,
}
impl Check for ProjectBranch {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<String>>(obj, "branch", &mut out);
        crate::types::keys::req_key::<bool>(obj, "folder", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <Option<String> as Check>::describe(rest),
                "folder" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:255 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub path: String,
    pub computer: String,
}
impl Check for ProjectRef {
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
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "computer", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:262 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonKind {
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "place")]
    Place,
}
impl Check for DaemonKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["cloud", "local", "place"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:270 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceOrigin {
    #[serde(rename = "here")]
    Here,
    #[serde(rename = "relayed")]
    Relayed,
    #[serde(rename = "paired")]
    Paired,
}
impl Check for WorkspaceOrigin {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["here", "relayed", "paired"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:276 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadScope {
    pub kind: ThreadLit,
    pub thread_id: String,
    pub workspace_id: String,
    pub root_thread_id: String,
}
impl Check for ThreadScope {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ThreadLit>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "rootThreadId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <ThreadLit as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "rootThreadId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:287 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventAsker {
    pub thread_id: String,
    pub root_thread_id: String,
}
impl Check for EventAsker {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "rootThreadId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "rootThreadId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:304 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAgents {
    pub max_machines: JsInt,
    pub max_depth: JsInt,
}
impl Check for WorkspaceAgents {
    /// Rules: maxMachines: >= 0; maxDepth: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.max_machines.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        crate::types::check::num_at_least(self.max_depth.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::req_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "maxMachines" => <JsInt as Check>::describe(rest),
                "maxDepth" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl WorkspaceAgents {
    fn rule_max_machines(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_max_depth(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.as_f64(), 1.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:316 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CopyRoad {
    #[serde(rename = "clonefile")]
    Clonefile,
    #[serde(rename = "worktree")]
    Worktree,
}
impl Check for CopyRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["clonefile", "worktree"]))
    }
}

impl Check for Carried {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["deps-and-config", "config-only", "nothing"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:327 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyAsk {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base: Option<String>,
    pub exclude: Vec<String>,
    pub size_line_bytes: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<CopyRoad>,
}
impl Check for CopyAsk {
    /// Rules: sizeLineBytes: >= 0.
    /// Nested checks: road.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.size_line_bytes.0 .0, 0.0).map_err(|e| e.under("sizeLineBytes"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "from", &mut out);
        crate::types::keys::req_key::<String>(obj, "to", &mut out);
        crate::types::keys::opt_key::<String>(obj, "base", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "exclude", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "sizeLineBytes", &mut out, Self::rule_size_line_bytes);
        crate::types::keys::opt_key::<CopyRoad>(obj, "road", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "to" => <String as Check>::describe(rest),
                "base" => <Option<String> as Check>::describe(rest),
                "exclude" => <Vec<String> as Check>::describe(rest),
                "sizeLineBytes" => <i64 as Check>::describe(rest),
                "road" => <Option<CopyRoad> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl CopyAsk {
    fn rule_size_line_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:343 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReport {
    pub road: CopyRoad,
    pub path: String,
    pub base: String,
    pub branch: String,
    pub fetched: bool,
    pub carried: Carried,
    pub excluded: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub skipped: Option<Vec<String>>,
    pub bytes: JsInt,
    pub ms: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fell_back: Option<String>,
}
impl Check for CopyReport {
    /// Rules: bytes: >= 0; ms: >= 0.
    /// Nested checks: road, carried.
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        self.carried.check().map_err(|e| e.under("carried"))?;
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::num_at_least(self.ms.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<CopyRoad>(obj, "road", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "base", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<bool>(obj, "fetched", &mut out);
        crate::types::keys::req_key::<Carried>(obj, "carried", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "excluded", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "skipped", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::req_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        crate::types::keys::opt_key::<String>(obj, "fellBack", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "road" => <CopyRoad as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "fetched" => <bool as Check>::describe(rest),
                "carried" => <Carried as Check>::describe(rest),
                "excluded" => <Vec<String> as Check>::describe(rest),
                "skipped" => <Option<Vec<String>> as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                "fellBack" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl CopyReport {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:364 at a3b870a6e.
/// One module a worktree carries in beside the config files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarryModule {
    pub id: String,
    pub lockfiles: Vec<String>,
    pub carry: Vec<String>,
    pub never: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installed: Option<String>,
}
impl Check for CarryModule {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "lockfiles", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "carry", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "never", &mut out);
        crate::types::keys::opt_key::<String>(obj, "installed", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "lockfiles" => <Vec<String> as Check>::describe(rest),
                "carry" => <Vec<String> as Check>::describe(rest),
                "never" => <Vec<String> as Check>::describe(rest),
                "installed" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:371 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeAsk {
    pub from: String,
    pub home: String,
    pub project: String,
    pub branch: String,
    pub modules: Vec<CarryModule>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkpoint: Option<String>,
}
impl Check for WorktreeAsk {
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        self.modules.check().map_err(|e| e.under("modules"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "from", &mut out);
        crate::types::keys::req_key::<String>(obj, "home", &mut out);
        crate::types::keys::req_key::<String>(obj, "project", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<Vec<CarryModule>>(obj, "modules", &mut out);
        crate::types::keys::opt_key::<String>(obj, "checkpoint", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "home" => <String as Check>::describe(rest),
                "project" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "modules" => <Vec<CarryModule> as Check>::describe(rest),
                "checkpoint" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:385 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopy {
    pub path: String,
    pub base: String,
    pub branch: String,
    pub carried: Carried,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fell_back: Option<String>,
    pub road: CopyRoad,
    pub source: String,
}
impl Check for ProjectCopy {
    /// Nested checks: carried, road.
    fn check(&self) -> Result<(), SchemaError> {
        self.carried.check().map_err(|e| e.under("carried"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "base", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<Carried>(obj, "carried", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fellBack", &mut out);
        crate::types::keys::req_key::<CopyRoad>(obj, "road", &mut out);
        crate::types::keys::req_key::<String>(obj, "source", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "carried" => <Carried as Check>::describe(rest),
                "fellBack" => <Option<String> as Check>::describe(rest),
                "road" => <CopyRoad as Check>::describe(rest),
                "source" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorktreeSettledWhy {
    #[serde(rename = "merged")]
    Merged,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "removed")]
    Removed,
}
impl Check for WorktreeSettledWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["merged", "closed", "deleted", "removed"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:394 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSettled {
    pub at: JsNumber,
    pub why: WorktreeSettledWhy,
}
impl Check for WorktreeSettled {
    fn check(&self) -> Result<(), SchemaError> {
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::req_key::<WorktreeSettledWhy>(obj, "why", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "why" => <WorktreeSettledWhy as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:402 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeFolder {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    pub made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub made_for: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settled: Option<WorktreeSettled>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub remove_failed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<True>,
}
impl Check for WorktreeFolder {
    /// Nested checks: settled.
    fn check(&self) -> Result<(), SchemaError> {
        self.settled.check().map_err(|e| e.under("settled"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<bool>(obj, "made", &mut out);
        crate::types::keys::opt_key::<String>(obj, "madeFor", &mut out);
        crate::types::keys::opt_key::<WorktreeSettled>(obj, "settled", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kept", &mut out);
        crate::types::keys::opt_key::<String>(obj, "removeFailed", &mut out);
        crate::types::keys::opt_key::<True>(obj, "gone", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                "madeFor" => <Option<String> as Check>::describe(rest),
                "settled" => <Option<WorktreeSettled> as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                "removeFailed" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<True> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:416 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundModule {
    pub id: String,
    pub folder: String,
    pub rebuild: bool,
}
impl Check for FoundModule {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "folder", &mut out);
        crate::types::keys::req_key::<bool>(obj, "rebuild", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                "rebuild" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:424 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeReport {
    pub path: String,
    pub branch: String,
    pub made: bool,
    pub carried: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub overlaid: Option<Vec<String>>,
    pub fresh: bool,
    pub modules: Vec<FoundModule>,
    pub ms: JsInt,
}
impl Check for WorktreeReport {
    /// Rules: ms: >= 0.
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        self.modules.check().map_err(|e| e.under("modules"))?;
        crate::types::check::num_at_least(self.ms.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<bool>(obj, "made", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "carried", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "plain", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "overlaid", &mut out);
        crate::types::keys::req_key::<bool>(obj, "fresh", &mut out);
        crate::types::keys::req_key::<Vec<FoundModule>>(obj, "modules", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                "carried" => <Vec<String> as Check>::describe(rest),
                "plain" => <Option<Vec<String>> as Check>::describe(rest),
                "overlaid" => <Option<Vec<String>> as Check>::describe(rest),
                "fresh" => <bool as Check>::describe(rest),
                "modules" => <Vec<FoundModule> as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl WorktreeReport {
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:439 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoval {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rescued: Option<String>,
}
impl Check for WorktreeRemoval {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rescued", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "rescued" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:444 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMade {
    pub path: String,
    pub branch: String,
    pub made: bool,
}
impl Check for WorktreeMade {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<bool>(obj, "made", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceViewQueueWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}
impl Check for WorkspaceViewQueueWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["slot", "minute"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewQueue {
    pub since: String,
    pub why: WorkspaceViewQueueWhy,
}
impl Check for WorkspaceViewQueue {
    fn check(&self) -> Result<(), SchemaError> {
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "since", &mut out);
        crate::types::keys::req_key::<WorkspaceViewQueueWhy>(obj, "why", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "since" => <String as Check>::describe(rest),
                "why" => <WorkspaceViewQueueWhy as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewScreen {
    pub stream_url: String,
}
impl Check for WorkspaceViewScreen {
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
pub struct WorkspaceViewDaemonRefusedAt {
    pub machine_id: String,
    pub at: String,
    pub why: String,
}
impl Check for WorkspaceViewDaemonRefusedAt {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::req_key::<String>(obj, "why", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machineId" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "why" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:454 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch_of_record: Option<BranchOfRecord>,
}
impl Check for WorkspaceView {
    /// Nested checks: phase, kind, project, queue, screen, theme, glyph, daemonRefusedAt, agents, worktree, pauseMode, from, review, branchOfRecord.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.queue.check().map_err(|e| e.under("queue"))?;
        self.screen.check().map_err(|e| e.under("screen"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.branch_of_record.check().map_err(|e| e.under("branchOfRecord"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<WorkspacePhase>(obj, "phase", &mut out);
        crate::types::keys::opt_key::<WorkspaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "golden", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<ProjectRef>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "home", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewQueue>(obj, "queue", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewScreen>(obj, "screen", &mut out);
        crate::types::keys::opt_key::<String>(obj, "gone", &mut out);
        crate::types::keys::opt_key::<WorkspaceTheme>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
        crate::types::keys::opt_key::<String>(obj, "daemonNote", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultRefused", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewDaemonRefusedAt>(obj, "daemonRefusedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "wakeRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "agents", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentWorkspaceId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
        crate::types::keys::opt_key::<WorktreeFolder>(obj, "worktree", &mut out);
        crate::types::keys::opt_key::<String>(obj, "provider", &mut out);
        crate::types::keys::opt_key::<PauseMode>(obj, "pauseMode", &mut out);
        crate::types::keys::opt_key::<WorkspaceFrom>(obj, "from", &mut out);
        crate::types::keys::opt_key::<ReviewDraft>(obj, "review", &mut out);
        crate::types::keys::opt_key::<BranchOfRecord>(obj, "branchOfRecord", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "queue" => <Option<WorkspaceViewQueue> as Check>::describe(rest),
                "screen" => <Option<WorkspaceViewScreen> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "branchOfRecord" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:553 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineFacts {
    pub os: String,
    pub uptime_ms: JsNumber,
    pub folder: String,
}
impl Check for MachineFacts {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "os", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "uptimeMs", &mut out);
        crate::types::keys::req_key::<String>(obj, "folder", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "os" => <String as Check>::describe(rest),
                "uptimeMs" => <JsNumber as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusWakeAsk {
    pub ask: JsNumber,
    pub of: JsNumber,
}
impl Check for WorkspaceStatusWakeAsk {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "ask", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "of", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ask" => <JsNumber as Check>::describe(rest),
                "of" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
