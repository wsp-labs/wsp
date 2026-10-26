// SPDX-License-Identifier: AGPL-3.0-only
//! `portProbe`'s fetch (workspaces.ts:1190-1202): a GET that follows no redirect, ended by `PORT_PROBE_TIMEOUT_MS`, with
//! the body read up to `PORT_PROBE_BODY_CAP` bytes (`readBodyUpTo`, internal.ts:1034).
use std::time::Duration;

use futures::StreamExt;
use wsp_core::types::internal::read_body_up_to;
use wsp_core::types::wiring::{PORT_PROBE_BODY_CAP, PORT_PROBE_TIMEOUT_MS};
use wsp_engine::fetch::{Abort, Fetch, FetchRequest};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::workspace::PortProbeView;
use wsp_words::exit::HostError;

/// `fetch(url, {redirect: "manual", signal: AbortSignal.timeout(PORT_PROBE_TIMEOUT_MS)})` then `readBodyUpTo(res, cap)`.
/// `fetch` is the platform's with redirects off (`platform_fetch_manual`); a failure answers fetch's own words.
pub async fn probe_port(fetch: &dyn Fetch, url: &str) -> Result<PortProbeView, HostError> {
    let abort = Abort::new(Some(Duration::from_millis(PORT_PROBE_TIMEOUT_MS)), None);
    let req = FetchRequest { method: "GET".into(), url: url.into(), headers: vec![], body: None, abort: Some(abort) };
    let res = fetch.fetch_stream(req).await.map_err(|e| HostError::new(e.message))?;
    let body = match res.body {
        Some(stream) => read_body_up_to(&mut stream.map(|chunk| chunk.map_err(|e| HostError::new(e.message))), PORT_PROBE_BODY_CAP).await,
        None => String::new(),
    };
    Ok(PortProbeView { status: JsInt(JsNumber(f64::from(res.status))), body })
}
