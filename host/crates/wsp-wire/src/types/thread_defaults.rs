// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/thread-defaults.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use indexmap::IndexMap;

/// TS: packages/protocol/src/thread-defaults.ts:18 at a3b870a6e.
pub const ACCESS_CHOICES: &[&str] = &["ask", "auto-edit", "full", "plan"];

/// TS: packages/protocol/src/thread-defaults.ts:307 at a3b870a6e.
pub const BUILT_IN_LIST_CLAUSE: &str =
    "; that list is wsp's built-in one, since no agent there described itself, and the agent on that computer may take more";

/// TS: packages/protocol/src/thread-defaults.ts:309 at a3b870a6e.
pub const BUILT_IN_TABLE_CLAUSE: &str = "; wsp's built-in table says so, since no agent there described itself";

/// TS: packages/protocol/src/thread-defaults.ts:394 at a3b870a6e.
pub const ENV_REFUSED_FIX: &str = "Name another variable; this one is the computer's to say.";

/// TS: packages/protocol/src/thread-defaults.ts:439 at a3b870a6e.
pub const ENV_VALUE_REFUSAL: &str =
    "only a socket holding this host's own token may set a variable's value; use wsp agents setup at the computer the host runs on";

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

impl Check for AccessChoice {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ask", "auto-edit", "full", "plan"]))
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:24 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPicker {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hide: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub custom: Option<Vec<String>>,
}

impl Check for ModelPicker {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<Vec<String>>(obj, "hide", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "order", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "custom", &mut out);
        let known: &[&str] = &["hide", "order", "custom"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "hide" => <Option<Vec<String>> as Check>::describe(rest),
                "order" => <Option<Vec<String>> as Check>::describe(rest),
                "custom" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub access: Option<AccessChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub models: Option<ModelPicker>,
}

impl Check for AgentDefaults {
    /// Nested checks: access, models.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.access.check().map_err(|e| e.under("access"))?;
        self.models.check().map_err(|e| e.under("models"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
        crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
        crate::types::keys::opt_key::<ModelPicker>(obj, "models", &mut out);
        let known: &[&str] = &["model", "effort", "access", "models"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "model" => <Option<String> as Check>::describe(rest),
                "effort" => <Option<String> as Check>::describe(rest),
                "access" => <Option<AccessChoice> as Check>::describe(rest),
                "models" => <Option<ModelPicker> as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.access.check().map_err(|e| e.under("access"))?;
        self.models.check().map_err(|e| e.under("models"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::dbl_key::<String>(obj, "model", &mut out);
        crate::types::keys::dbl_key::<String>(obj, "effort", &mut out);
        crate::types::keys::dbl_key::<AccessChoice>(obj, "access", &mut out);
        crate::types::keys::dbl_key::<ModelPicker>(obj, "models", &mut out);
        let known: &[&str] = &["model", "effort", "access", "models"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "model" => <Option<Option<String>> as Check>::describe(rest),
                "effort" => <Option<Option<String>> as Check>::describe(rest),
                "access" => <Option<Option<AccessChoice>> as Check>::describe(rest),
                "models" => <Option<Option<ModelPicker>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:40 at a3b870a6e.
/// TS: thread-defaults.ts:40 at a3b870a6e..
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackerRepo(pub String);

impl Check for TrackerRepo {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::regex_check(crate::types::check::tracker_repo(&self.0))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub access: Option<AccessChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::trimmed_opt")]
    pub after_worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub trackers: Option<Vec<TrackerRepo>>,
}

impl Check for ProjectOverrides {
    /// Rules: afterWorktree: zod string check trim; afterWorktree: at least 1 UTF-16 units.
    /// Nested checks: access, trackers.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.access.check().map_err(|e| e.under("access"))?;
        if let Some(v) = &self.after_worktree {
            crate::types::check::text_at_least_with(
                v,
                1,
                "the after-worktree command is a shell line; --reset after-worktree takes it away",
            )
            .map_err(|e| e.under("afterWorktree"))?;
        }
        self.trackers.check().map_err(|e| e.under("trackers"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
        crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
        crate::types::keys::opt_trimmed_with(obj, "afterWorktree", &mut out, Self::rule_after_worktree);
        crate::types::keys::opt_key::<Vec<TrackerRepo>>(obj, "trackers", &mut out);
        let known: &[&str] = &["agent", "model", "effort", "access", "afterWorktree", "trackers"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <Option<String> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "effort" => <Option<String> as Check>::describe(rest),
                "access" => <Option<AccessChoice> as Check>::describe(rest),
                "afterWorktree" => <Option<String> as Check>::describe(rest),
                "trackers" => <Option<Vec<TrackerRepo>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ProjectOverrides {
    fn rule_after_worktree(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least_with(
                v,
                1,
                "the after-worktree command is a shell line; --reset after-worktree takes it away",
            )?;
        }
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option_trimmed")]
    pub after_worktree: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub trackers: Option<Option<Vec<TrackerRepo>>>,
}

impl Check for ProjectOverridesPatch {
    /// Rules: afterWorktree: zod string check trim; afterWorktree: at least 1 UTF-16 units.
    /// Nested checks: access, trackers.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.access.check().map_err(|e| e.under("access"))?;
        if let Some(Some(v)) = &self.after_worktree {
            crate::types::check::text_at_least_with(
                v,
                1,
                "the after-worktree command is a shell line; --reset after-worktree takes it away",
            )
            .map_err(|e| e.under("afterWorktree"))?;
        }
        self.trackers.check().map_err(|e| e.under("trackers"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::dbl_key::<String>(obj, "agent", &mut out);
        crate::types::keys::dbl_key::<String>(obj, "model", &mut out);
        crate::types::keys::dbl_key::<String>(obj, "effort", &mut out);
        crate::types::keys::dbl_key::<AccessChoice>(obj, "access", &mut out);
        crate::types::keys::dbl_trimmed_with(obj, "afterWorktree", &mut out, Self::rule_after_worktree);
        crate::types::keys::dbl_key::<Vec<TrackerRepo>>(obj, "trackers", &mut out);
        let known: &[&str] = &["agent", "model", "effort", "access", "afterWorktree", "trackers"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <Option<Option<String>> as Check>::describe(rest),
                "model" => <Option<Option<String>> as Check>::describe(rest),
                "effort" => <Option<Option<String>> as Check>::describe(rest),
                "access" => <Option<Option<AccessChoice>> as Check>::describe(rest),
                "afterWorktree" => <Option<Option<String>> as Check>::describe(rest),
                "trackers" => <Option<Option<Vec<TrackerRepo>>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ProjectOverridesPatch {
    fn rule_after_worktree(x: &Option<Option<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = x {
            crate::types::check::text_at_least_with(
                v,
                1,
                "the after-worktree command is a shell line; --reset after-worktree takes it away",
            )?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:58 at a3b870a6e.
/// TS: thread-defaults.ts:58 at a3b870a6e..
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnvName(pub String);

impl Check for EnvName {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::regex_check_with(
            crate::types::check::env_name(&self.0),
            "a variable's name is letters, digits and _, not starting with a digit",
        )
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:63 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSetupView {
    pub on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub args: Option<Vec<String>>,
    pub env_names: Vec<String>,
}

impl Check for AgentSetupView {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<bool>(obj, "on", &mut out);
        crate::types::keys::opt_key::<String>(obj, "program", &mut out);
        crate::types::keys::opt_key::<String>(obj, "configDir", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "args", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "envNames", &mut out);
        let known: &[&str] = &["on", "program", "configDir", "args", "envNames"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "on" => <bool as Check>::describe(rest),
                "program" => <Option<String> as Check>::describe(rest),
                "configDir" => <Option<String> as Check>::describe(rest),
                "args" => <Option<Vec<String>> as Check>::describe(rest),
                "envNames" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:69 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSetupSet {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub program: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub config_dir: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub args: Option<Option<Vec<String>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::check::present",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub env: Option<IndexMap<EnvName, Option<String>>>,
}

impl Check for AgentSetupSet {
    /// Rules: program: at least 1 UTF-16 units; configDir: at least 1 UTF-16 units.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(Some(v)) = &self.program {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("program"))?;
        }
        if let Some(Some(v)) = &self.config_dir {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("configDir"))?;
        }
        self.env.check().map_err(|e| e.under("env"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<bool>(obj, "on", &mut out);
        crate::types::keys::dbl_key_with::<String>(obj, "program", &mut out, Self::rule_program);
        crate::types::keys::dbl_key_with::<String>(obj, "configDir", &mut out, Self::rule_config_dir);
        crate::types::keys::dbl_key::<Vec<String>>(obj, "args", &mut out);
        crate::types::keys::opt_key::<IndexMap<EnvName, Option<String>>>(obj, "env", &mut out);
        let known: &[&str] = &["on", "program", "configDir", "args", "env"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "on" => <Option<bool> as Check>::describe(rest),
                "program" => <Option<Option<String>> as Check>::describe(rest),
                "configDir" => <Option<Option<String>> as Check>::describe(rest),
                "args" => <Option<Option<Vec<String>>> as Check>::describe(rest),
                "env" => <Option<IndexMap<EnvName, Option<String>>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl AgentSetupSet {
    fn rule_program(x: &Option<Option<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
    }
    fn rule_config_dir(x: &Option<Option<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
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

impl Check for DefaultFrom {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["named", "project", "default", "catalog"]))
    }
}

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
        self.from.check().map_err(|e| e.under("from"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "value", &mut out);
        crate::types::keys::req_key::<DefaultFrom>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "value" => <String as Check>::describe(rest),
                "from" => <DefaultFrom as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.value.check().map_err(|e| e.under("value"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<AccessChoice>(obj, "value", &mut out);
        crate::types::keys::req_key::<String>(obj, "mode", &mut out);
        crate::types::keys::req_key::<DefaultFrom>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "value" => <AccessChoice as Check>::describe(rest),
                "mode" => <String as Check>::describe(rest),
                "from" => <DefaultFrom as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/thread-defaults.ts:130 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDefaults {
    pub agent: ResolvedPick,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<ResolvedPick>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<ResolvedPick>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub access: Option<ResolvedAccess>,
}

impl Check for ThreadDefaults {
    /// Nested checks: agent, model, effort, access.
    fn check(&self) -> Result<(), SchemaError> {
        self.agent.check().map_err(|e| e.under("agent"))?;
        self.model.check().map_err(|e| e.under("model"))?;
        self.effort.check().map_err(|e| e.under("effort"))?;
        self.access.check().map_err(|e| e.under("access"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ResolvedPick>(obj, "agent", &mut out);
        crate::types::keys::opt_key::<ResolvedPick>(obj, "model", &mut out);
        crate::types::keys::opt_key::<ResolvedPick>(obj, "effort", &mut out);
        crate::types::keys::opt_key::<ResolvedAccess>(obj, "access", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <ResolvedPick as Check>::describe(rest),
                "model" => <Option<ResolvedPick> as Check>::describe(rest),
                "effort" => <Option<ResolvedPick> as Check>::describe(rest),
                "access" => <Option<ResolvedAccess> as Check>::describe(rest),
                _ => None,
            },
        }
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
    let mut next = match kept.map(serde_json::to_value) {
        Some(Ok(serde_json::Value::Object(fields))) => fields,
        _ => serde_json::Map::new(),
    };
    if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(patch) {
        for (key, value) in fields {
            match value {
                serde_json::Value::Null => {
                    next.shift_remove(&key);
                }
                value => {
                    next.insert(key, value);
                }
            }
        }
    }
    if next.is_empty() {
        return None;
    }
    serde_json::from_value(serde_json::Value::Object(next)).ok()
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on: Option<wsp_frames::False>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub config_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::check::present",
        serialize_with = "crate::types::check::js_record_opt"
    )]
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
