// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/app-procs.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/views/app-procs.ts:19 at a3b870a6e.
pub const APP_PROC_ORIGINS_MAX: u64 = 20;

/// TS: packages/protocol/src/views/app-procs.ts:21 at a3b870a6e.
pub const APP_PROC_ORIGIN_CHARS: u64 = 256;

/// TS: packages/protocol/src/views/app-procs.ts:23 at a3b870a6e.
pub const APP_PROCS_MAX: u64 = 200;

/// TS: packages/protocol/src/views/app-procs.ts:47 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const APP_PROC_WORDS: &[(&str, &str)] = &[
    ("main", "App"),
    ("gpu", "Graphics"),
    ("network", "Network"),
    ("renderer", "Window"),
    ("tab", "Browser tab"),
    ("frame", "Frame"),
    ("helper", "Helper"),
];

/// TS: packages/protocol/src/views/app-procs.ts:15 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppProcKind {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "gpu")]
    Gpu,
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "renderer")]
    Renderer,
    #[serde(rename = "tab")]
    Tab,
    #[serde(rename = "frame")]
    Frame,
    #[serde(rename = "helper")]
    Helper,
}

impl Check for AppProcKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["main", "gpu", "network", "renderer", "tab", "frame", "helper"]))
    }
}

/// TS: packages/protocol/src/views/app-procs.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProc {
    pub pid: i64,
    pub kind: AppProcKind,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub origins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub contents: Option<Vec<i64>>,
}

impl Check for AppProc {
    /// Rules: pid: > 0; origins[]: at most 256 UTF-16 units; origins: at most 20 items; contents[]: >= 0; contents: at most 20 items.
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.pid as f64, 0.0).map_err(|e| e.under("pid"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        if let Some(list) = &self.origins {
            for (i, item) in list.iter().enumerate() {
                crate::types::check::text_at_most(item, 256).map_err(|e| e.under(i).under("origins"))?;
            }
        }
        if let Some(list) = &self.origins {
            crate::types::check::items_at_most(list.len(), 20).map_err(|e| e.under("origins"))?;
        }
        if let Some(list) = &self.contents {
            for (i, &item) in list.iter().enumerate() {
                crate::types::check::num_at_least(item as f64, 0.0).map_err(|e| e.under(i).under("contents"))?;
            }
        }
        if let Some(list) = &self.contents {
            crate::types::check::items_at_most(list.len(), 20).map_err(|e| e.under("contents"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "kind" => <AppProcKind as Check>::describe(rest),
                "origins" => <Option<Vec<String>> as Check>::describe(rest),
                "contents" => <Option<Vec<i64>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/app-procs.ts:37 at a3b870a6e.
/// TS: packages/protocol/src/views/app-procs.ts:37 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppProcList(pub Vec<AppProc>);

impl Check for AppProcList {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::items_at_most(self.0.len(), 200)?;
        self.0.check()
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        <Vec<AppProc> as Check>::describe(path)
    }
}

/// TS: packages/protocol/src/views/app-procs.ts:40 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProcsEvent {
    pub procs: AppProcList,
}

impl AppProcsEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "app.procs";
}

impl Check for AppProcsEvent {
    /// Nested checks: procs.
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "procs" => <AppProcList as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/app-procs.ts:44 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProcsWantedEvent {
    pub wanted: bool,
}

impl AppProcsWantedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "app.procs.wanted";
}

impl Check for AppProcsWantedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "wanted" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
