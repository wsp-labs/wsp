// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/sub-row.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use wsp_frames::True;

/// TS: packages/protocol/src/views/sub-row.ts:55 at a3b870a6e.
pub const SUB_SPAWN_LOG_LINES: u64 = 40;

/// TS: packages/protocol/src/views/sub-row.ts:12 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubResult {
    pub branch: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub pushed_sha: Option<String>,
    pub commits: i64,
    pub unpushed: i64,
    pub changed: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub norepo: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_failed: Option<String>,
    pub read_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_refused: Option<String>,
}

impl Check for SubResult {
    /// Rules: commits: >= 0; unpushed: >= 0; changed: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnBranch {
    pub branch: String,
    pub sha: String,
    pub ms: i64,
}

impl Check for SubSpawnBranch {
    /// Rules: ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnMachine {
    pub since: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<i64>,
}

impl Check for SubSpawnMachine {
    /// Rules: ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnFiles {
    pub since: JsNumber,
    pub paths: Vec<String>,
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<i64>,
}

impl Check for SubSpawnFiles {
    /// Rules: bytes: >= 0; ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubSpawnFailedStep {
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "files")]
    Files,
}

impl Check for SubSpawnFailedStep {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnFailed {
    pub step: SubSpawnFailedStep,
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Check for SubSpawnFailed {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:34 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawn {
    pub started_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<SubSpawnBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<SubSpawnMachine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<SubSpawnFiles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<SubSpawnFailed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncommitted: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_sent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced: Option<Vec<String>>,
    pub log: Vec<String>,
}

impl Check for SubSpawn {
    /// Rules: uncommitted: >= 0.
    /// Nested checks: branch, machine, files, failed.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:62 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubState {
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "napping")]
    Napping,
    #[serde(rename = "released")]
    Released,
}

impl Check for SubState {}

/// TS: packages/protocol/src/views/sub-row.ts:66 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubHeld {
    pub files: i64,
    pub commits: i64,
}

impl Check for SubHeld {
    /// Rules: files: >= 0; commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRowSize {
    pub cpu: i64,
    pub mem_mb: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Check for SubRowSize {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:69 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRow {
    pub state: SubState,
    pub on: String,
    pub machine: String,
    pub size: SubRowSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_usd_per_hour: Option<JsNumber>,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SubResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<SubHeld>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_gone: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_not_kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<SubSpawn>,
}

impl Check for SubRow {
    /// Nested checks: state, size, result, held, spawn.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// ---- Hand-written below: the functions of the unit.

// views/sub-row.ts:58 at a3b870a6e
// export const subSpawning = (spawn: SubSpawn | undefined): boolean => spawn !== undefined && spawn.readyAt === undefined && spawn.failed === undefined;
pub fn sub_spawning(spawn: Option<&SubSpawn>) -> bool {
    todo!()
}

// views/sub-row.ts:93 at a3b870a6e
// export const subAllPushed = (r: SubResult): boolean => r.readFailed === undefined && r.unpushed === 0 && r.changed === 0;
pub fn sub_all_pushed(r: &SubResult) -> bool {
    todo!()
}

// views/sub-row.ts:97 at a3b870a6e
// export const subTurnGone = (sub: { releasedAt?: number } | undefined, startedAt: number | undefined): boolean =>
//   sub?.releasedAt !== undefined && (startedAt === undefined || startedAt < sub.releasedAt);
/// `released_at` is the sub-thread's, where there is one; `started_at` the turn's start.
pub fn sub_turn_gone(released_at: Option<f64>, started_at: Option<f64>) -> bool {
    todo!()
}
