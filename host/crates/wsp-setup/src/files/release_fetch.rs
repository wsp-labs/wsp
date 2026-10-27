// SPDX-License-Identifier: AGPL-3.0-only
//! `release-fetch.ts`: the fetch of the update road, with a connect bound, retries on a dropped connection and redirects followed here.
use std::rc::Rc;

use wsp_engine::fetch::Fetch;

/// `RELEASE_CONNECT_MS` (`release-fetch.ts:10`).
pub const RELEASE_CONNECT_MS: u64 = 30_000;

/// `RELEASE_ANSWER_MS` (`release-fetch.ts:12`).
pub const RELEASE_ANSWER_MS: u64 = 30_000;

/// `RELEASE_TRIES` (`release-fetch.ts:13`).
pub const RELEASE_TRIES: usize = 3;

/// `RELEASE_BACKOFF_MS` (`release-fetch.ts:14`).
pub const RELEASE_BACKOFF_MS: u64 = 1_000;

/// `ReleaseFetchOptions` (`release-fetch.ts:19`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseFetchOptions {
    pub connect_ms: Option<u64>,
    pub backoff_ms: Option<u64>,
    /// PEM certificates a local test server signs with, trusted in place of the system's.
    pub ca: Option<Vec<String>>,
}

/// `releaseFetch` (`release-fetch.ts:31`), the `typeof fetch` it returns.
pub fn release_fetch(_opts: ReleaseFetchOptions) -> Rc<dyn Fetch> {
    todo!()
}
