// SPDX-License-Identifier: AGPL-3.0-only
//! sub-spawn.ts:25-30, 76-281, 343-412: the record and every check before it. Spec: spec/sub_spawn_checks.md.
use std::sync::Arc;

use serde_json::Value;

use wsp_catalog::git_hosts::GitHost;
use wsp_engine::box_backend::BoxLimits;
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_words::exit::HostError;
use wsp_words::sub_thread::SubSize;

use super::{ImageRecord, Seed, SubSpawnArea};
use crate::machines::core_types::{AreaResult, Live};
use wsp_wire::views::golden_image::GoldenVersion;

/// sub-spawn.ts:26. The image every sub-machine forks.
pub const SETUP_IMAGE: &str = "default";
/// sub-spawn.ts:30. The states a sealed login stands in on a fork.
pub const LOGIN_HELD: [&str; 2] = ["copied", "signed-in"];

/// sub-spawn.ts:28. The words a git host's sign-in goes by.
pub fn host_words(host: &GitHost) -> &'static str {
    todo!()
}

/// What a start with `sub` asks (types/wiring.ts:98 `SubAsk`), beside the create's name and project word.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpawnAsk {
    pub name: String,
    pub project: String,
    pub on: String,
    pub size: Option<String>,
    pub from: Option<String>,
    pub branch: Option<String>,
    pub harness: Option<String>,
}

/// The sub half of the answer (types/wiring.ts `SubStarted`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubSpawned {
    pub out: SubOut,
    pub folder: String,
    pub preamble: String,
    pub harness: String,
}

/// `CreatedWorkspace` as a spawn answers it: the record's view, then `notice?`, then `sub`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnAnswer {
    pub view: Value,
    pub notice: Option<String>,
    pub sub: SubSpawned,
}

/// The image head a spawn forks and the image record beside it (sub-spawn.ts:104).
#[derive(Clone, Debug)]
pub struct ImageAt {
    pub head: GoldenVersion,
    pub record: Option<ImageRecord>,
}

impl SubSpawnArea {
    /// sub-spawn.ts:137 `spawnSub` (rules 2-5).
    pub async fn spawn_sub(&self, ask: SpawnAsk, origin: Option<&Caller>) -> Result<SpawnAnswer, HostError> {
        todo!()
    }

    /// sub-spawn.ts:77 `providerAt`.
    pub(crate) fn provider_at(&self, id: &str) -> Option<Arc<dyn MachineBackend>> {
        todo!()
    }

    /// sub-spawn.ts:80 (rule 6).
    pub(crate) fn sold_by(&self, at: &dyn MachineBackend, limits: Option<&BoxLimits>) -> Vec<SubSize> {
        todo!()
    }

    /// sub-spawn.ts:86 `settingOf(place, "size")`.
    pub(crate) fn kept_size(&self, place: &str) -> Option<Value> {
        todo!()
    }

    /// sub-spawn.ts:93 (rule 7).
    pub(crate) async fn host_signed_in(&self) -> bool {
        todo!()
    }

    /// sub-spawn.ts:104 (rule 8).
    pub(crate) async fn image_at(&self, place: &str, on: &str, host: &GitHost, remote: &str, brokered: bool) -> AreaResult<ImageAt> {
        todo!()
    }

    /// sub-spawn.ts:123 (rule 9).
    pub(crate) fn agent_on(&self, record: Option<&ImageRecord>, on: &str, harness: &str, brokered: bool) -> Result<(), HostError> {
        todo!()
    }

    /// sub-spawn.ts:131 (rule 10).
    pub(crate) fn starts_left(&self, limits: Option<&BoxLimits>, name: &str) -> Result<(), HostError> {
        todo!()
    }

    /// sub-spawn.ts:347 (rule 11).
    pub(crate) async fn free_branch(&self, wanted: &str, lead: &Live) -> String {
        todo!()
    }

    /// sub-spawn.ts:354 (rule 12).
    pub(crate) async fn git_at_lead(&self, lead: &Live, args: &[&str]) -> AreaResult<wsp_engine::machine::ExecResult> {
        todo!()
    }

    /// sub-spawn.ts:360 (rule 13).
    pub(crate) async fn remote_read(&self, lead: &Live, from: &str, args: &[&str]) -> Result<String, HostError> {
        todo!()
    }

    /// sub-spawn.ts:369 (rule 14). Answers `(branch, sha)`.
    pub(crate) async fn from_ref(&self, from: &str, lead: &Live, preferred: Option<&str>) -> Result<(String, String), HostError> {
        todo!()
    }

    /// sub-spawn.ts:392 (rule 15).
    pub(crate) async fn seed_of(&self, project: &ProjectView) -> AreaResult<Seed> {
        todo!()
    }
}
