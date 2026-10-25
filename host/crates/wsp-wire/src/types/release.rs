// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/release.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/release.ts:52 at a3b870a6e.
pub const UP_RESTART_LINE: &str = "Ctrl-C the terminal running wsp up and run it again.";

/// TS: packages/protocol/src/release.ts:55 at a3b870a6e.
pub const HOST_NO_RESTART_LINE: &str = "This host cannot restart itself.";

/// TS: packages/protocol/src/release.ts:58 at a3b870a6e.
pub const HOST_RESTART_TICKET_REFUSAL: &str = "a socket let in on a ticket cannot restart this host; restart it on the computer it runs on";

/// TS: packages/protocol/src/release.ts:9 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLatest {
    pub version: String,
    pub tag: String,
    pub url: String,
    pub published_at: String,
}

impl Check for ReleaseLatest {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "version", &mut out);
        crate::types::keys::req_key::<String>(obj, "tag", &mut out);
        crate::types::keys::req_key::<String>(obj, "url", &mut out);
        crate::types::keys::req_key::<String>(obj, "publishedAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "version" => <String as Check>::describe(rest),
                "tag" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "publishedAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/release.ts:14 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReleaseState {
    #[serde(rename = "checking")]
    Checking,
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "unreached")]
    Unreached,
    #[serde(rename = "off")]
    Off,
}

impl Check for ReleaseState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["checking", "read", "unreached", "off"]))
    }
}

/// TS: packages/protocol/src/release.ts:18 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostShape {
    #[serde(rename = "verb")]
    Verb,
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "service")]
    Service,
}

impl Check for HostShape {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["verb", "up", "service"]))
    }
}

/// TS: packages/protocol/src/release.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseView {
    pub state: ReleaseState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub latest: Option<ReleaseLatest>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tried_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shape: Option<HostShape>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub restart_refusal: Option<String>,
}

impl Check for ReleaseView {
    /// Nested checks: state, latest, shape.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.latest.check().map_err(|e| e.under("latest"))?;
        self.shape.check().map_err(|e| e.under("shape"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReleaseState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<ReleaseLatest>(obj, "latest", &mut out);
        crate::types::keys::opt_key::<String>(obj, "checkedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "triedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "installed", &mut out);
        crate::types::keys::opt_key::<String>(obj, "update", &mut out);
        crate::types::keys::opt_key::<HostShape>(obj, "shape", &mut out);
        crate::types::keys::opt_key::<String>(obj, "restartRefusal", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <ReleaseState as Check>::describe(rest),
                "latest" => <Option<ReleaseLatest> as Check>::describe(rest),
                "checkedAt" => <Option<String> as Check>::describe(rest),
                "triedAt" => <Option<String> as Check>::describe(rest),
                "installed" => <Option<String> as Check>::describe(rest),
                "update" => <Option<String> as Check>::describe(rest),
                "shape" => <Option<HostShape> as Check>::describe(rest),
                "restartRefusal" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/release.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseChangedEvent {
    pub release: ReleaseView,
}

impl ReleaseChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "release.changed";
}

impl Check for ReleaseChangedEvent {
    /// Nested checks: release.
    fn check(&self) -> Result<(), SchemaError> {
        self.release.check().map_err(|e| e.under("release"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReleaseView>(obj, "release", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "release" => <ReleaseView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
