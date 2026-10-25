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

impl Check for AppProcKind {}

/// TS: packages/protocol/src/views/app-procs.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProc {
    pub pid: i64,
    pub kind: AppProcKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<Vec<i64>>,
}

impl Check for AppProc {
    /// Rules: pid: > 0; origins[]: at most 256 UTF-16 units; origins: at most 20 items; contents[]: >= 0; contents: at most 20 items.
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/app-procs.ts:37 at a3b870a6e.
pub type AppProcList = Vec<AppProc>;
// Rules: AppProcList[]: check of AppProc; AppProcList: at most 200 items.

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
        todo!()
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
        todo!()
    }
}
