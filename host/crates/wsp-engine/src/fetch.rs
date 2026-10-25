// SPDX-License-Identifier: AGPL-3.0-only
//! `globalThis.fetch`, the one HTTP seam the provider backends (box-backend.ts, solari-backend.ts) take as an option
//! so a test hands in its own. No TS file of its own: the TS reads the platform's.
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::errors::RoadError;

/// What ends a fetch early (errors.ts:261's answer): the token fires when either the cap or the caller's signal does,
/// and `timed_out` tells which, so the failure reads "TimeoutError" for the cap and "AbortError" for the caller.
#[derive(Clone)]
pub struct Abort {
    pub token: CancellationToken,
    pub timed_out: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub struct FetchRequest {
    pub method: String,
    pub url: String,
    /// In the order the TS object literal writes them.
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub abort: Option<Abort>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl FetchResponse {
    /// `res.headers.get(name)`: case-insensitive, `None` where absent.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    /// `res.text()`: the body as UTF-8, invalid sequences replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// A fetch that rejects answers a [`RoadError`]: "fetch failed" with its cause, or a TimeoutError or AbortError.
#[async_trait]
pub trait Fetch: Send + Sync {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError>;
}
