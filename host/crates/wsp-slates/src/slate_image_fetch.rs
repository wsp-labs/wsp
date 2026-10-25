// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-image-fetch.ts at 65f6b89c7. Spec: spec/slate_images.md (rules 7-10).
//!
//! A slate's remote image, fetched by the host for the window. Every address a name resolves to is checked before
//! the socket opens and the socket dials the address checked; a redirect is followed only within the domain the
//! person allowed and checked again; no cookie or login goes with it; the bytes stop at the cap as they arrive.
#![allow(unused_variables)]

use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::pending::SlatesImageAnswer;

pub const REDIRECTS_MAX: u32 = 5;
pub const FETCH_MS: u64 = 15_000;

/// One address a name resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookedUp {
    pub address: String,
    /// 4 or 6.
    pub family: u8,
}

// slate-image-fetch.ts:367 localAddress(address)
/// Why an address is one this host will not dial for an image, or `None` for one on the open internet (rule 7).
pub fn local_address(address: &str) -> Option<String> {
    todo!()
}

/// slate-image-fetch.ts:376 FetchRoad.
pub trait FetchRoad {
    /// Every address a name resolves to; a failure reads as none.
    fn lookup(&self, host: &str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>>;
    /// Why an address may not be dialled, `None` where it may.
    fn refuses(&self, address: &str) -> Option<String>;
    fn timeout_ms(&self) -> u64;
}

// slate-image-fetch.ts:384 FETCH_ROAD
/// The system resolver, `local_address`, 15 s.
pub fn fetch_road() -> Rc<dyn FetchRoad> {
    todo!()
}

// slate-image-fetch.ts:451 fetchSlateImage(href, domain, road)
/// Fetches an image whose domain the person allowed (rule 9).
pub async fn fetch_slate_image(href: &str, domain: &str, road: Option<Rc<dyn FetchRoad>>) -> SlatesImageAnswer {
    todo!()
}
