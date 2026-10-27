// SPDX-License-Identifier: AGPL-3.0-only
//! `release.ts`: this host's reading of the newest release, asked on a timer and kept in release.json beside the state file.
use std::rc::Rc;

use wsp_engine::fetch::{Fetch, FetchResponse};
use wsp_serve::serve::ReleaseDoor;
use wsp_wire::types::release::{HostShape, ReleaseLatest, ReleaseState};
use wsp_words::exit::HostError;

use crate::context::Env;
use crate::files::release_fetch::{RELEASE_ANSWER_MS, RELEASE_CONNECT_MS, RELEASE_TRIES};

/// `RELEASE_API` (`release.ts:19`).
pub const RELEASE_API: &str = "https://api.github.com";

/// `RELEASE_FIRST_MS` (`release.ts:20`).
pub const RELEASE_FIRST_MS: u64 = 30_000;

/// `RELEASE_EVERY_MS` (`release.ts:21`).
pub const RELEASE_EVERY_MS: u64 = 6 * 60 * 60_000;

/// `RELEASE_FLOOR_MS` (`release.ts:23`).
pub const RELEASE_FLOOR_MS: u64 = 10 * 60_000;

/// `RELEASE_TIMEOUT_MS` (`release.ts:25`).
pub const RELEASE_TIMEOUT_MS: u64 = RELEASE_TRIES as u64 * RELEASE_CONNECT_MS + RELEASE_ANSWER_MS + 10_000;

/// `RELEASE_BODY_MAX_BYTES` (`release.ts:27`).
pub const RELEASE_BODY_MAX_BYTES: u64 = 1024 * 1024;

/// `releaseUrl` (`release.ts:35`).
pub fn release_url(_env: &Env) -> String {
    todo!()
}

/// `releaseTagUrl` (`release.ts:38`).
pub fn release_tag_url(_env: &Env, _tag: &str) -> String {
    todo!()
}

/// `releaseAssetUrl` (`release.ts:39`).
pub fn release_asset_url(_env: &Env, _tag: &str, _asset: &str) -> String {
    todo!()
}

/// `releaseFileFor` (`release.ts:42`).
pub fn release_file_for(_state_path: &str) -> String {
    todo!()
}

/// `parseRelease` (`release.ts:48`), GitHub's answer as a release, or the refusal for a draft, a prerelease or a stray tag.
pub fn parse_release(_body: &serde_json::Value) -> Result<ReleaseLatest, HostError> {
    todo!()
}

/// `cappedText` (`release.ts:56`).
pub async fn capped_text(_res: &FetchResponse, _max: u64) -> Result<String, HostError> {
    todo!()
}

/// `updateChecksOff` (`release.ts:86`).
pub fn update_checks_off(_state_path: &str, _env: &Env) -> bool {
    todo!()
}

/// `ReleaseReading` (`release.ts:89`), the `Pick` of `ReleaseView` for `state` and `latest`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseReading {
    pub state: ReleaseState,
    pub latest: Option<ReleaseLatest>,
}

/// `releaseReading` (`release.ts:90`).
pub fn release_reading(_state_path: &str, _env: &Env) -> Option<ReleaseReading> {
    todo!()
}

/// `latestWords` (`release.ts:98`).
pub fn latest_words(_reading: &ReleaseReading, _running: &str, _fix: Rc<dyn Fn(&str) -> String>) -> String {
    todo!()
}

/// `ReleaseWatchRestart` (`release.ts:110`), the inline `restart` of `ReleaseWatchOptions`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseWatchRestart {
    pub refusal: Option<String>,
}

/// `ReleaseWatchOptions` (`release.ts:103`).
pub struct ReleaseWatchOptions {
    pub state_path: String,
    pub shape: Option<HostShape>,
    pub running: String,
    pub installed: Rc<dyn Fn() -> String>,
    pub restart: Option<ReleaseWatchRestart>,
    pub update: Option<Rc<dyn Fn(&str) -> String>>,
    pub env: Option<Env>,
    pub fetch: Option<Rc<dyn Fetch>>,
    pub now: Option<Rc<dyn Fn() -> i64>>,
    pub log: Option<Rc<dyn Fn(&str)>>,
}

/// `ReleaseWatch` (`release.ts:121`), a release door that also starts its timer and closes it.
pub trait ReleaseWatch: ReleaseDoor {
    /// `ReleaseWatch.start` (`release.ts:122`).
    fn start(&self);
    /// `ReleaseWatch.close` (`release.ts:123`).
    fn close(&self);
}

/// `releaseWatch` (`release.ts:126`).
pub fn release_watch(_opts: ReleaseWatchOptions) -> Rc<dyn ReleaseWatch> {
    todo!()
}
